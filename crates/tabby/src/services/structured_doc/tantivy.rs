//! Поиск структурированных документов через Tantivy
//!
//! Реализует семантический поиск документов с использованием векторных эмбеддингов
//! и полнотекстового индекса Tantivy. Обеспечивает быстрый поиск по локальным данным
//! с фильтрацией по источникам и ранжированием по релевантности.

// === IMPORTS ===
use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, ConstScoreQuery, Occur, Query},
    schema::{self, Value},
    IndexReader, TantivyDocument,
};
use tracing::warn;

use tabby_common::{
    api::structured_doc::{
        DocSearch, DocSearchDocument, DocSearchError, DocSearchHit, DocSearchResponse,
        FromTantivyDocument,
    },
    index::{self, corpus},
};
use tabby_inference::Embedding;

use crate::services::tantivy::IndexReaderProvider;

// === CONSTANTS ===
/// Минимальный порог оценки эмбеддинга для включения результата в ответ
///
/// Результаты с оценкой ниже этого порога отфильтровываются как нерелевантные.
/// Значение 0.75 обеспечивает баланс между точностью и полнотой результатов.
const EMBEDDING_SCORE_THRESHOLD: f32 = 0.75;

// === STRUCTS ===
/// Внутренняя реализация поиска документов
///
/// Содержит логику семантического поиска с использованием эмбеддингов
/// и взаимодействия с индексом Tantivy. Отделена от публичного API
/// для упрощения тестирования и переиспользования.
struct DocSearchImpl {
    /// Сервис для генерации векторных эмбеддингов из текстовых запросов
    embedding: Arc<dyn Embedding>,
}

/// Промежуточная структура для хранения оценённого чанка документа
///
/// Используется в процессе обработки результатов поиска для связывания
/// фрагментов документов с их оценками релевантности и метаданными.
struct ScoredChunk {
    /// Уникальный идентификатор документа
    doc_id: String,
    /// Оценка релевантности фрагмента (от 0.0 до 1.0)
    score: f32,
    /// Tantivy документ с содержимым фрагмента
    chunk: TantivyDocument,
}

/// Сервис поиска структурированных документов
///
/// Предоставляет API для семантического поиска документов в локальном
/// индексе Tantivy. Использует векторные эмбеддинги для определения
/// релевантности и поддерживает фильтрацию по источникам данных.
pub struct DocSearchService {
    /// Внутренняя реализация логики поиска
    imp: DocSearchImpl,
    /// Провайдер для доступа к индексу Tantivy
    provider: Arc<IndexReaderProvider>,
}

// === IMPLEMENTATIONS ===
impl DocSearchImpl {
    /// Создаёт новый экземпляр внутренней реализации поиска
    ///
    /// # Arguments
    ///
    /// * `embedding` - Сервис для генерации векторных эмбеддингов
    fn new(embedding: Arc<dyn Embedding>) -> Self {
        Self { embedding }
    }

    /// Выполняет семантический поиск документов в индексе
    ///
    /// Процесс поиска включает:
    /// 1. Генерацию эмбеддинга для поискового запроса
    /// 2. Построение составного запроса с фильтрами
    /// 3. Поиск релевантных фрагментов документов
    /// 4. Дедупликацию и ранжирование результатов
    /// 5. Получение полных документов для найденных фрагментов
    ///
    /// # Arguments
    ///
    /// * `source_ids` - Список идентификаторов источников для фильтрации
    /// * `reader` - Читатель индекса Tantivy
    /// * `q` - Текст поискового запроса
    /// * `limit` - Максимальное количество результатов
    ///
    /// # Returns
    ///
    /// Ответ с найденными документами, отсортированными по релевантности
    ///
    /// # Errors
    ///
    /// Возвращает ошибку если:
    /// - Не удалось сгенерировать эмбеддинг для запроса
    /// - Ошибка при выполнении поиска в индексе
    /// - Ошибка при получении документов из индекса
    async fn search(
        &self,
        source_ids: &[String],
        reader: &IndexReader,
        q: &str,
        limit: usize,
    ) -> Result<DocSearchResponse, DocSearchError> {
        let schema = index::IndexSchema::instance();
        
        // Строим составной запрос с эмбеддингами и фильтрами
        let query = {
            let embedding = self.embedding.embed(q).await?;
            let embedding_tokens_query =
                index::embedding_tokens_query(embedding.len(), embedding.iter());
            let corpus_query = schema.corpus_query(corpus::STRUCTURED_DOC);

            let mut query_clauses: Vec<(Occur, Box<dyn Query>)> = vec![
                (
                    Occur::Must,
                    Box::new(ConstScoreQuery::new(corpus_query, 0.0)),
                ),
                (Occur::Must, Box::new(embedding_tokens_query)),
            ];

            // Добавляем фильтр по источникам, если указаны
            if !source_ids.is_empty() {
                let source_ids_query = Box::new(schema.source_ids_query(source_ids));
                let source_ids_query = ConstScoreQuery::new(source_ids_query, 0.0);
                query_clauses.push((Occur::Must, Box::new(source_ids_query)));
            }
            
            BooleanQuery::new(query_clauses)
        };

        let searcher = reader.searcher();
        // Запрашиваем больше результатов для последующей дедупликации
        let top_chunks = searcher.search(&query, &TopDocs::with_limit(limit * 2))?;

        // Обрабатываем найденные фрагменты
        let chunks = {
            // Извлекаем все фрагменты с метаданными
            let mut chunks: Vec<_> = top_chunks
                .iter()
                .filter_map(|(score, chunk_address)| {
                    let chunk: TantivyDocument = searcher.doc(*chunk_address).ok()?;
                    let doc_id = get_text(&chunk, schema.field_id).to_owned();
                    Some(ScoredChunk {
                        score: *score,
                        chunk,
                        doc_id,
                    })
                })
                .collect();

            // Сортируем по убыванию релевантности
            chunks.sort_unstable_by(|lhs, rhs| rhs.score.total_cmp(&lhs.score));

            // Удаляем дубликаты по идентификатору документа
            let mut doc_ids = HashSet::new();
            chunks.retain(|x| doc_ids.insert(x.doc_id.clone()));

            chunks
        };

        // Получаем полные документы для найденных фрагментов
        let hits = chunks
            .iter()
            .filter_map(
                |ScoredChunk {
                     doc_id,
                     score,
                     chunk,
                 }| {
                    // Ищем полный документ по идентификатору
                    let doc_query = schema.doc_query(corpus::STRUCTURED_DOC, doc_id);
                    let top_docs = match searcher.search(&doc_query, &TopDocs::with_limit(1)) {
                        Err(err) => {
                            warn!("Не удалось найти документ `{}`: `{}`", doc_id, err);
                            return None;
                        }
                        Ok(top_docs) => top_docs,
                    };
                    
                    let (_, doc_address) = top_docs.first()?;
                    let doc: TantivyDocument = searcher.doc(*doc_address).ok()?;
                    
                    // Преобразуем в унифицированный формат
                    DocSearchDocument::from_tantivy_document(&doc, chunk)
                        .map(|doc| DocSearchHit { score: *score, doc })
                },
            )
            // Фильтруем по минимальному порогу релевантности
            .filter(|x| x.score >= EMBEDDING_SCORE_THRESHOLD)
            .take(limit)
            .collect();

        Ok(DocSearchResponse { hits })
    }
}

impl DocSearchService {
    /// Создаёт новый сервис поиска документов
    ///
    /// # Arguments
    ///
    /// * `embedding` - Сервис для генерации векторных эмбеддингов
    /// * `provider` - Провайдер для доступа к индексу Tantivy
    ///
    /// # Returns
    ///
    /// Новый экземпляр сервиса поиска
    pub fn new(embedding: Arc<dyn Embedding>, provider: Arc<IndexReaderProvider>) -> Self {
        Self {
            imp: DocSearchImpl::new(embedding),
            provider,
        }
    }
}

#[async_trait]
impl DocSearch for DocSearchService {
    /// Выполняет поиск документов с фильтрацией по источникам
    ///
    /// Реализует трейт `DocSearch` для интеграции с общей системой поиска.
    /// Проверяет готовность индекса перед выполнением поиска.
    ///
    /// # Arguments
    ///
    /// * `source_ids` - Список идентификаторов источников для фильтрации
    /// * `q` - Текст поискового запроса
    /// * `limit` - Максимальное количество результатов
    ///
    /// # Returns
    ///
    /// Ответ с найденными документами или ошибка
    ///
    /// # Errors
    ///
    /// Возвращает `DocSearchError::NotReady` если индекс недоступен
    async fn search(
        &self,
        source_ids: &[String],
        q: &str,
        limit: usize,
    ) -> Result<DocSearchResponse, DocSearchError> {
        if let Some(reader) = self.provider.reader().await.as_ref() {
            self.imp.search(source_ids, reader, q, limit).await
        } else {
            Err(DocSearchError::NotReady)
        }
    }
}

// === FREE FUNCTIONS ===
/// Извлекает текстовое значение поля из Tantivy документа
///
/// Вспомогательная функция для безопасного извлечения строковых значений
/// из документов Tantivy. Предполагает, что поле существует и содержит строку.
///
/// # Arguments
///
/// * `doc` - Tantivy документ
/// * `field` - Поле схемы для извлечения
///
/// # Returns
///
/// Текстовое значение поля
///
/// # Panics
///
/// Паникует если поле отсутствует или содержит не строковое значение
fn get_text(doc: &TantivyDocument, field: schema::Field) -> &str {
    doc.get_first(field).unwrap().as_str().unwrap()
}
