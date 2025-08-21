//! Сервис поиска кода с поддержкой гибридного ранжирования
//!
//! Реализует интеллектуальный поиск по кодовой базе, комбинируя embedding-поиск
//! и BM25 алгоритм для максимальной точности результатов. Использует RRF (Reciprocal
//! Rank Fusion) для объединения результатов разных методов поиска.

// === IMPORTS ===
use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use tantivy::{
    collector::TopDocs,
    schema::{self, Value},
    IndexReader, TantivyDocument,
};

use tabby_common::{
    api::code::{
        CodeSearch, CodeSearchDocument, CodeSearchError, CodeSearchHit, CodeSearchParams,
        CodeSearchQuery, CodeSearchResponse, CodeSearchScores,
    },
    index::{
        self,
        code::{self, tokenize_code},
        corpus, IndexSchema,
    },
};
use tabby_inference::Embedding;

use super::tantivy::IndexReaderProvider;

// === CONSTANTS ===
/// Константа для вычисления RRF (Reciprocal Rank Fusion) скора
///
/// Используется в формуле: 1.0 / (RANK_CONSTANT + rank + 1)
/// Значение 60.0 обеспечивает сбалансированное влияние позиции в ранжировании
const RANK_CONSTANT: f32 = 60.0;

// === STRUCTS ===
/// Внутренняя реализация поиска кода
///
/// Выполняет фактический поиск по индексу, используя embedding модель
/// для семантического поиска и BM25 для лексического поиска.
/// Результаты объединяются с помощью RRF алгоритма.
struct CodeSearchImpl {
    /// Модель для создания векторных представлений кода
    embedding: Arc<dyn Embedding>,
}

/// Публичный сервис поиска кода
///
/// Предоставляет API для поиска релевантных фрагментов кода
/// в индексированной кодовой базе. Автоматически обрабатывает
/// недоступность индекса и возвращает соответствующие ошибки.
///
/// # Examples
///
/// ```rust
/// let service = CodeSearchService::new(embedding, provider);
/// let query = CodeSearchQuery::new("function implementation");
/// let params = CodeSearchParams::default();
/// let results = service.search_in_language(query, params).await?;
/// ```
struct CodeSearchService {
    /// Внутренняя реализация поиска
    imp: CodeSearchImpl,
    
    /// Провайдер доступа к индексу
    provider: Arc<IndexReaderProvider>,
}

// === IMPLEMENTATIONS ===
impl CodeSearchImpl {
    /// Создает новую реализацию поиска кода
    ///
    /// # Arguments
    ///
    /// * `embedding` - Модель для создания векторных представлений
    fn new(embedding: Arc<dyn Embedding>) -> Self {
        Self { embedding }
    }

    /// Выполняет поиск по индексу с заданным запросом
    ///
    /// Использует Tantivy для выполнения поискового запроса и возвращает
    /// отсортированные по релевантности результаты с их скорами.
    ///
    /// # Arguments
    ///
    /// * `reader` - Читатель индекса Tantivy
    /// * `q` - Поисковый запрос
    /// * `limit` - Максимальное количество результатов
    ///
    /// # Returns
    ///
    /// Вектор кортежей (скор, документ) отсортированных по релевантности
    ///
    /// # Errors
    ///
    /// Возвращает `CodeSearchError` при ошибках поиска в индексе
    async fn search_with_query(
        &self,
        reader: &IndexReader,
        q: &dyn tantivy::query::Query,
        limit: usize,
    ) -> Result<Vec<(f32, TantivyDocument)>, CodeSearchError> {
        let searcher = reader.searcher();
        let top_docs = { searcher.search(q, &(TopDocs::with_limit(limit)))? };
        let top_docs = top_docs
            .iter()
            .map(|(score, doc_address)| {
                let doc: TantivyDocument = searcher.doc(*doc_address).unwrap();
                (*score, doc)
            })
            .collect();
        Ok(top_docs)
    }

    /// Выполняет гибридный поиск кода по языку программирования
    ///
    /// Комбинирует два метода поиска:
    /// 1. Embedding-поиск для семантического понимания
    /// 2. BM25 поиск для точного лексического соответствия
    /// 
    /// Результаты объединяются с помощью RRF алгоритма для получения
    /// наиболее релевантных фрагментов кода.
    ///
    /// # Arguments
    ///
    /// * `reader` - Читатель индекса
    /// * `query` - Поисковый запрос с контекстом
    /// * `params` - Параметры поиска и фильтрации
    ///
    /// # Returns
    ///
    /// Ответ с отранжированными результатами поиска
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при проблемах с embedding моделью или индексом
    async fn search_in_language(
        &self,
        reader: &IndexReader,
        query: CodeSearchQuery,
        params: CodeSearchParams,
    ) -> Result<CodeSearchResponse, CodeSearchError> {
        // Поиск через embedding модель для семантического понимания
        let docs_from_embedding = {
            let embedding = self.embedding.embed(&query.content).await?;
            let embedding_tokens_query = Box::new(index::embedding_tokens_query(
                embedding.len(),
                embedding.iter(),
            ));

            let query = code::code_search_query(&query, embedding_tokens_query);
            self.search_with_query(reader, &query, params.num_to_score)
                .await?
        };

        // Поиск через BM25 для точного лексического соответствия
        let docs_from_bm25 = {
            let body_tokens = tokenize_code(&query.content);
            let body_query = code::body_query(&body_tokens);

            let query = code::code_search_query(&query, body_query);
            self.search_with_query(reader, &query, params.num_to_score)
                .await?
        };

        Ok(
            merge_code_responses_by_rank(reader, &params, docs_from_embedding, docs_from_bm25)
                .await,
        )
    }
}

impl CodeSearchService {
    /// Создает новый сервис поиска кода
    ///
    /// # Arguments
    ///
    /// * `embedding` - Модель для векторных представлений
    /// * `provider` - Провайдер доступа к индексу
    pub fn new(embedding: Arc<dyn Embedding>, provider: Arc<IndexReaderProvider>) -> Self {
        Self {
            imp: CodeSearchImpl::new(embedding),
            provider,
        }
    }
}

#[async_trait]
impl CodeSearch for CodeSearchService {
    /// Выполняет поиск кода по языку программирования
    ///
    /// Проверяет доступность индекса и делегирует поиск внутренней реализации.
    /// Возвращает ошибку `NotReady` если индекс недоступен.
    ///
    /// # Arguments
    ///
    /// * `query` - Поисковый запрос
    /// * `params` - Параметры поиска
    ///
    /// # Returns
    ///
    /// Результаты поиска или ошибку о недоступности
    async fn search_in_language(
        &self,
        query: CodeSearchQuery,
        params: CodeSearchParams,
    ) -> Result<CodeSearchResponse, CodeSearchError> {
        if let Some(reader) = self.provider.reader().await.as_ref() {
            self.imp.search_in_language(reader, query, params).await
        } else {
            Err(CodeSearchError::NotReady)
        }
    }
}

// === FREE FUNCTIONS ===
/// Создает сервис поиска кода
///
/// Фабричная функция для инициализации сервиса поиска с заданными
/// зависимостями. Возвращает реализацию трейта `CodeSearch`.
///
/// # Arguments
///
/// * `embedding` - Модель для создания векторных представлений
/// * `provider` - Провайдер доступа к поисковому индексу
///
/// # Returns
///
/// Реализация трейта CodeSearch готовая к использованию
///
/// # Examples
///
/// ```rust
/// let embedding = load_embedding_model().await;
/// let provider = Arc::new(IndexReaderProvider::new());
/// let search_service = create_code_search(embedding, provider);
/// ```
pub fn create_code_search(
    embedding: Arc<dyn Embedding>,
    provider: Arc<IndexReaderProvider>,
) -> impl CodeSearch {
    CodeSearchService::new(embedding, provider)
}

/// Объединяет результаты embedding и BM25 поиска с помощью RRF
///
/// Использует алгоритм Reciprocal Rank Fusion для комбинирования результатов
/// двух разных методов поиска. Применяет фильтрацию по минимальным скорам
/// и ограничивает количество результатов на файл для разнообразия.
///
/// # Arguments
///
/// * `reader` - Читатель индекса для получения метаданных
/// * `params` - Параметры фильтрации и ограничений
/// * `embedding_resp` - Результаты embedding поиска
/// * `bm25_resp` - Результаты BM25 поиска
///
/// # Returns
///
/// Объединенный и отфильтрованный ответ с результатами поиска
async fn merge_code_responses_by_rank(
    reader: &IndexReader,
    params: &CodeSearchParams,
    embedding_resp: Vec<(f32, TantivyDocument)>,
    bm25_resp: Vec<(f32, TantivyDocument)>,
) -> CodeSearchResponse {
    let mut scored_hits: HashMap<String, (CodeSearchScores, TantivyDocument)> = HashMap::default();

    // Обрабатываем результаты embedding поиска
    for (rank, embedding, doc) in compute_rank_score(embedding_resp).into_iter() {
        let scores = CodeSearchScores {
            rrf: rank,
            embedding,
            ..Default::default()
        };

        scored_hits.insert(get_chunk_id(&doc).to_owned(), (scores, doc));
    }

    // Обрабатываем результаты BM25 поиска и объединяем скоры
    for (rank, bm25, doc) in compute_rank_score(bm25_resp).into_iter() {
        let chunk_id = get_chunk_id(&doc);
        if let Some((score, _)) = scored_hits.get_mut(chunk_id) {
            score.rrf += rank;
            score.bm25 = bm25;
        } else {
            let scores = CodeSearchScores {
                rrf: rank,
                bm25,
                ..Default::default()
            };
            scored_hits.insert(chunk_id.to_owned(), (scores, doc));
        }
    }

    // Создаем финальные результаты поиска
    let scored_hits_futures: Vec<_> = scored_hits
        .into_values()
        .map(|(scores, doc)| create_hit(reader, scores, doc))
        .collect();
    let mut scored_hits: Vec<CodeSearchHit> = futures::future::join_all(scored_hits_futures)
        .await
        .into_iter()
        .collect();
    
    // Сортируем по RRF скору (убывание)
    scored_hits.sort_by(|a, b| b.scores.rrf.total_cmp(&a.scores.rrf));
    
    // Ограничиваем количество результатов на файл для разнообразия
    retain_at_most_two_hits_per_file(&mut scored_hits);

    CodeSearchResponse {
        hits: scored_hits
            .into_iter()
            .filter(|hit| {
                hit.scores.bm25 > params.min_bm25_score
                    && hit.scores.embedding > params.min_embedding_score
                    && hit.scores.rrf > params.min_rrf_score
            })
            .take(params.num_to_return)
            .collect(),
    }
}

/// Ограничивает количество результатов на файл для разнообразия
///
/// Удаляет избыточные результаты из одного файла, оставляя максимум
/// 2 фрагмента на файл. Это обеспечивает разнообразие результатов
/// и предотвращает доминирование одного файла в выдаче.
///
/// # Arguments
///
/// * `scored_hits` - Мутабельный вектор результатов для фильтрации
fn retain_at_most_two_hits_per_file(scored_hits: &mut Vec<CodeSearchHit>) {
    let mut scored_hits_by_fileid: HashMap<String, usize> = HashMap::default();
    scored_hits.retain(|x| {
        let count: usize = scored_hits_by_fileid
            .get(&x.doc.file_id)
            .copied()
            .unwrap_or_default();
        scored_hits_by_fileid.insert(x.doc.file_id.clone(), count + 1);
        count < 2
    });
}

/// Вычисляет RRF скор на основе позиции в ранжировании
///
/// Преобразует результаты поиска в формат (rrf_score, original_score, document)
/// используя формулу RRF: 1.0 / (RANK_CONSTANT + rank + 1).
/// Это обеспечивает сбалансированное влияние позиции в итоговом ранжировании.
///
/// # Arguments
///
/// * `resp` - Результаты поиска с оригинальными скорами
///
/// # Returns
///
/// Вектор с RRF скорами, оригинальными скорами и документами
fn compute_rank_score(resp: Vec<(f32, TantivyDocument)>) -> Vec<(f32, f32, TantivyDocument)> {
    resp.into_iter()
        .enumerate()
        .map(|(rank, (score, doc))| (1.0 / (RANK_CONSTANT + (rank + 1) as f32), score, doc))
        .collect()
}

/// Извлекает идентификатор чанка из документа
///
/// # Arguments
///
/// * `doc` - Документ Tantivy для извлечения ID
///
/// # Returns
///
/// Строковый идентификатор чанка кода
fn get_chunk_id(doc: &TantivyDocument) -> &str {
    let schema = IndexSchema::instance();
    get_text(doc, schema.field_chunk_id)
}

/// Создает результат поиска из документа и скоров
///
/// Извлекает все необходимые поля из документа Tantivy и создает
/// структуру CodeSearchHit с полной информацией о найденном фрагменте кода.
///
/// # Arguments
///
/// * `reader` - Читатель индекса для получения дополнительных данных
/// * `scores` - Скоры релевантности
/// * `doc` - Документ Tantivy с данными
///
/// # Returns
///
/// Полностью сформированный результат поиска
async fn create_hit(
    reader: &IndexReader,
    scores: CodeSearchScores,
    doc: TantivyDocument,
) -> CodeSearchHit {
    let schema = IndexSchema::instance();
    let file_id = get_text(&doc, schema.field_id).to_owned();
    let commit = get_commit(reader, &file_id).await;

    let doc = CodeSearchDocument {
        file_id,
        chunk_id: get_text(&doc, schema.field_chunk_id).to_owned(),
        body: get_json_text_field(
            &doc,
            schema.field_chunk_attributes,
            code::fields::CHUNK_BODY,
        )
        .to_owned(),
        filepath: get_json_text_field(
            &doc,
            schema.field_chunk_attributes,
            code::fields::CHUNK_FILEPATH,
        )
        .to_owned(),
        git_url: get_json_text_field(
            &doc,
            schema.field_chunk_attributes,
            code::fields::CHUNK_GIT_URL,
        )
        .to_owned(),
        // commit добавлен в v0.23, но это обязательное поле
        // поэтому нужно обрабатывать случай его отсутствия
        commit,
        language: get_json_text_field(
            &doc,
            schema.field_chunk_attributes,
            code::fields::CHUNK_LANGUAGE,
        )
        .to_owned(),
        start_line: get_optional_json_number_field(
            &doc,
            schema.field_chunk_attributes,
            code::fields::CHUNK_START_LINE,
        ),
    };
    CodeSearchHit { scores, doc }
}

/// Получает информацию о коммите для файла
///
/// Выполняет дополнительный поиск в индексе для получения SHA коммита,
/// связанного с файлом. Возвращает None если информация недоступна.
///
/// # Arguments
///
/// * `reader` - Читатель индекса
/// * `id` - Идентификатор файла
///
/// # Returns
///
/// Опциональный SHA коммита
async fn get_commit(reader: &IndexReader, id: &str) -> Option<String> {
    let schema = IndexSchema::instance();
    let query = schema.doc_query(corpus::CODE, id);
    let doc = reader
        .searcher()
        .search(&query, &TopDocs::with_limit(1))
        .ok()?;
    if doc.is_empty() {
        return None;
    }

    let doc = reader.searcher().doc(doc[0].1).ok()?;
    get_json_text_field_optional(&doc, schema.field_attributes, code::fields::COMMIT)
        .map(|s| s.to_owned())
}

/// Извлекает текстовое значение поля из документа
///
/// # Arguments
///
/// * `doc` - Документ Tantivy
/// * `field` - Поле схемы для извлечения
///
/// # Returns
///
/// Строковое значение поля
fn get_text(doc: &TantivyDocument, field: schema::Field) -> &str {
    doc.get_first(field).unwrap().as_str().unwrap()
}

/// Извлекает опциональное числовое поле из JSON атрибутов
///
/// # Arguments
///
/// * `doc` - Документ Tantivy
/// * `field` - Поле с JSON данными
/// * `name` - Имя поля в JSON
///
/// # Returns
///
/// Опциональное числовое значение
fn get_optional_json_number_field(
    doc: &TantivyDocument,
    field: schema::Field,
    name: &str,
) -> Option<usize> {
    doc.get_first(field)
        .unwrap()
        .as_object()
        .unwrap()
        .find(|(k, _)| *k == name)?
        .1
        .as_i64()
        .map(|x| x as usize)
}

/// Извлекает обязательное текстовое поле из JSON атрибутов
///
/// # Arguments
///
/// * `doc` - Документ Tantivy
/// * `field` - Поле с JSON данными
/// * `name` - Имя поля в JSON
///
/// # Returns
///
/// Строковое значение поля
fn get_json_text_field<'a>(doc: &'a TantivyDocument, field: schema::Field, name: &str) -> &'a str {
    doc.get_first(field)
        .unwrap()
        .as_object()
        .unwrap()
        .find(|(k, _)| *k == name)
        .unwrap()
        .1
        .as_str()
        .unwrap()
}

/// Извлекает опциональное текстовое поле из JSON атрибутов
///
/// # Arguments
///
/// * `doc` - Документ Tantivy
/// * `field` - Поле с JSON данными
/// * `name` - Имя поля в JSON
///
/// # Returns
///
/// Опциональное строковое значение
fn get_json_text_field_optional<'a>(
    doc: &'a TantivyDocument,
    field: schema::Field,
    name: &str,
) -> Option<&'a str> {
    doc.get_first(field)
        .and_then(|value| value.as_object())
        .and_then(|mut obj| obj.find(|(k, _)| *k == name))
        .and_then(|(_, v)| v.as_str())
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;

    /// Тестирует функцию ограничения результатов на файл
    ///
    /// Проверяет корректность работы алгоритма, который оставляет
    /// максимум 2 результата на файл для обеспечения разнообразия выдачи.
    #[test]
    fn test_retain_at_most_two_hits_per_file() {
        /// Создает тестовый результат поиска
        let new_hit = |file: &str, body: &str| CodeSearchHit {
            scores: Default::default(),
            doc: CodeSearchDocument {
                file_id: file.to_string(),
                chunk_id: "chunk1".to_owned(),
                body: body.to_string(),
                filepath: "".to_owned(),
                git_url: "".to_owned(),
                commit: Some("".to_owned()),
                language: "".to_owned(),
                start_line: Some(0),
            },
        };

        let cases = vec![
            (vec![], vec![]),
            (
                vec![new_hit("file1", "body1")],
                vec![new_hit("file1", "body1")],
            ),
            (
                vec![new_hit("file1", "body1"), new_hit("file1", "body2")],
                vec![new_hit("file1", "body1"), new_hit("file1", "body2")],
            ),
            (
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file1", "body3"),
                ],
                vec![new_hit("file1", "body1"), new_hit("file1", "body2")],
            ),
            (
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file1", "body3"),
                    new_hit("file2", "body4"),
                ],
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file2", "body4"),
                ],
            ),
            (
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file1", "body3"),
                    new_hit("file2", "body4"),
                    new_hit("file2", "body5"),
                ],
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file2", "body4"),
                    new_hit("file2", "body5"),
                ],
            ),
            (
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file1", "body3"),
                    new_hit("file2", "body4"),
                    new_hit("file2", "body5"),
                    new_hit("file2", "body6"),
                ],
                vec![
                    new_hit("file1", "body1"),
                    new_hit("file1", "body2"),
                    new_hit("file2", "body4"),
                    new_hit("file2", "body5"),
                ],
            ),
        ];

        for (input, expected) in cases {
            let mut input = input;
            retain_at_most_two_hits_per_file(&mut input);
            assert_eq!(input, expected);
        }
    }
}
