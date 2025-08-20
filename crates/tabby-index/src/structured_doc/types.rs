// === MODULES ===
pub mod commit;
pub mod ingested;
pub mod issue;
pub mod page;
pub mod pull;
pub mod web;

// === IMPORTS ===
use std::sync::Arc;

use anyhow::{bail, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use tabby_inference::Embedding;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::indexer::{IndexId, ToIndexId};

// === CONSTANTS ===
/// Константы типов документов для индексации
///
/// Используются для категоризации документов в поисковом индексе
/// и обеспечения единообразной типизации различных источников контента.
pub const KIND_WEB: &str = "web";
pub const KIND_ISSUE: &str = "issue";
pub const KIND_PULL: &str = "pull";
pub const KIND_COMMIT: &str = "commit";
pub const KIND_PAGE: &str = "page";
pub const KIND_INGESTED: &str = "ingested";

// === STRUCTS ===
/// Структурированный документ для индексации
///
/// Представляет единую абстракцию над различными типами документов
/// (веб-страницы, issues, pull requests, коммиты, страницы документации).
/// Обеспечивает унифицированный интерфейс для индексации контента
/// из разнообразных источников.
///
/// # Поля
///
/// * `source_id` - Идентификатор источника документа (репозиторий, сайт)
/// * `fields` - Специфичные для типа поля документа
///
/// # Examples
///
/// ```rust
/// let doc = StructuredDoc {
///     source_id: "github.com/user/repo".to_string(),
///     fields: StructuredDocFields::Issue(issue_doc),
/// };
/// ```
pub struct StructuredDoc {
    pub source_id: String,
    pub fields: StructuredDocFields,
}

// === ENUMS ===
/// Типы полей структурированных документов
///
/// Перечисление всех поддерживаемых типов документов в системе индексации.
/// Каждый вариант содержит специфичную для типа структуру данных
/// с соответствующими полями и методами обработки.
///
/// # Варианты
///
/// * `Web` - Веб-страницы и HTML документы
/// * `Issue` - Issues из систем управления проектами
/// * `Pull` - Pull requests и merge requests
/// * `Commit` - Git коммиты с метаданными
/// * `Page` - Страницы документации
/// * `Ingested` - Предварительно обработанные документы
pub enum StructuredDocFields {
    Web(web::WebDocument),
    Issue(issue::IssueDocument),
    Pull(pull::PullDocument),
    Commit(commit::CommitDocument),
    Page(page::PageDocument),
    Ingested(ingested::IngestedDocument),
}

// === TRAITS ===
/// Трейт для построения структурированных документов с embedding'ами
///
/// Определяет интерфейс для обработки документов перед индексацией,
/// включая проверку необходимости пропуска, построение атрибутов
/// и создание chunk'ов с векторными представлениями.
///
/// # Lifetime Parameters
///
/// * `'content_chunks` - Время жизни для потоков chunk'ов контента
#[async_trait]
pub trait BuildStructuredDoc<'content_chunks> {
    /// Проверяет, следует ли пропустить обработку документа
    ///
    /// Используется для фильтрации документов на основе различных критериев:
    /// размера, типа контента, даты последнего изменения и других факторов.
    ///
    /// # Returns
    ///
    /// `true` если документ должен быть пропущен, `false` для обработки
    fn should_skip(&self) -> bool;

    /// Строит атрибуты документа для индексации
    ///
    /// Создает JSON объект с метаданными документа, включая заголовки,
    /// описания, авторов, даты и другую структурированную информацию
    /// для поисковых запросов и фильтрации.
    ///
    /// # Returns
    ///
    /// JSON объект с атрибутами документа
    async fn build_attributes(&self) -> serde_json::Value;
    
    /// Строит атрибуты для chunk'ов контента с embedding'ами
    ///
    /// Разбивает документ на семантические chunk'и, создает для каждого
    /// векторное представление и соответствующие атрибуты. Возвращает
    /// поток асинхронных задач для параллельной обработки.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Сервис для создания векторных представлений
    ///
    /// # Returns
    ///
    /// Поток задач, каждая из которых возвращает токены embedding'а и атрибуты
    async fn build_chunk_attributes(
        &self,
        embedding: Arc<dyn Embedding>,
    ) -> BoxStream<'content_chunks, JoinHandle<Result<(Vec<String>, serde_json::Value)>>>;
}

// === IMPLEMENTATIONS ===
impl StructuredDoc {
    /// Возвращает уникальный идентификатор документа
    ///
    /// Извлекает ID из соответствующего типа документа.
    /// Для разных типов используются разные поля как идентификаторы:
    /// ссылки для веб-документов, SHA для коммитов, и т.д.
    ///
    /// # Returns
    ///
    /// Строковый идентификатор документа
    pub fn id(&self) -> &str {
        match &self.fields {
            StructuredDocFields::Web(web) => &web.link,
            StructuredDocFields::Issue(issue) => &issue.link,
            StructuredDocFields::Pull(pull) => &pull.link,
            StructuredDocFields::Commit(commit) => &commit.sha,
            StructuredDocFields::Page(page) => &page.link,
            StructuredDocFields::Ingested(ingested) => &ingested.id,
        }
    }

    /// Возвращает тип документа как строковую константу
    ///
    /// Определяет категорию документа для индексации и поиска.
    /// Используется для фильтрации результатов по типам источников.
    ///
    /// # Returns
    ///
    /// Строковая константа типа документа
    pub fn kind(&self) -> &'static str {
        match &self.fields {
            StructuredDocFields::Web(_) => KIND_WEB,
            StructuredDocFields::Issue(_) => KIND_ISSUE,
            StructuredDocFields::Pull(_) => KIND_PULL,
            StructuredDocFields::Commit(_) => KIND_COMMIT,
            StructuredDocFields::Page(_) => KIND_PAGE,
            StructuredDocFields::Ingested(_) => KIND_INGESTED,
        }
    }
}

impl ToIndexId for StructuredDoc {
    /// Преобразует документ в идентификатор для индекса
    ///
    /// Создает составной идентификатор из источника и ID документа
    /// для уникальной идентификации в поисковом индексе.
    ///
    /// # Returns
    ///
    /// IndexId с source_id и id документа
    fn to_index_id(&self) -> IndexId {
        IndexId {
            source_id: self.source_id.clone(),
            id: self.id().to_owned(),
        }
    }
}

#[async_trait]
impl<'content_chunks> BuildStructuredDoc<'content_chunks> for StructuredDoc {
    /// Делегирует проверку пропуска соответствующему типу документа
    fn should_skip(&self) -> bool {
        match &self.fields {
            StructuredDocFields::Web(doc) => doc.should_skip(),
            StructuredDocFields::Issue(doc) => doc.should_skip(),
            StructuredDocFields::Pull(doc) => doc.should_skip(),
            StructuredDocFields::Commit(doc) => doc.should_skip(),
            StructuredDocFields::Page(doc) => doc.should_skip(),
            StructuredDocFields::Ingested(doc) => doc.should_skip(),
        }
    }

    /// Делегирует построение атрибутов соответствующему типу документа
    async fn build_attributes(&self) -> serde_json::Value {
        match &self.fields {
            StructuredDocFields::Web(doc) => doc.build_attributes().await,
            StructuredDocFields::Issue(doc) => doc.build_attributes().await,
            StructuredDocFields::Pull(doc) => doc.build_attributes().await,
            StructuredDocFields::Commit(doc) => doc.build_attributes().await,
            StructuredDocFields::Page(doc) => doc.build_attributes().await,
            StructuredDocFields::Ingested(doc) => doc.build_attributes().await,
        }
    }

    /// Делегирует построение chunk атрибутов соответствующему типу документа
    async fn build_chunk_attributes(
        &self,
        embedding: Arc<dyn Embedding>,
    ) -> BoxStream<'content_chunks, JoinHandle<Result<(Vec<String>, serde_json::Value)>>> {
        match &self.fields {
            StructuredDocFields::Web(doc) => doc.build_chunk_attributes(embedding).await,
            StructuredDocFields::Issue(doc) => doc.build_chunk_attributes(embedding).await,
            StructuredDocFields::Pull(doc) => doc.build_chunk_attributes(embedding).await,
            StructuredDocFields::Commit(doc) => doc.build_chunk_attributes(embedding).await,
            StructuredDocFields::Page(doc) => doc.build_chunk_attributes(embedding).await,
            StructuredDocFields::Ingested(doc) => doc.build_chunk_attributes(embedding).await,
        }
    }
}

// === FREE FUNCTIONS ===
/// Создает токены embedding'а из текста
///
/// Преобразует текстовый контент в векторное представление и затем
/// в бинаризованные токены для эффективного хранения и поиска.
/// Используется для создания поисковых индексов на основе семантического сходства.
///
/// # Arguments
///
/// * `embedding` - Сервис для создания векторных представлений
/// * `text` - Текст для обработки
///
/// # Returns
///
/// Вектор строковых токенов представляющих embedding
///
/// # Errors
///
/// Возвращает ошибку если:
/// - Сервис embedding'а недоступен
/// - Текст не может быть обработан
/// - Ошибка бинаризации векторного представления
async fn build_tokens(embedding: Arc<dyn Embedding>, text: &str) -> Result<Vec<String>> {
    // Создаем векторное представление текста
    let embedding = match embedding.embed(text).await {
        Ok(embedding) => embedding,
        Err(err) => {
            warn!("Failed to embed chunk text: {}", err);
            bail!("Failed to embed chunk text: {}", err);
        }
    };

    // Бинаризуем embedding для эффективного хранения и поиска
    let mut chunk_embedding_tokens = vec![];
    for token in tabby_common::index::binarize_embedding(embedding.iter()) {
        chunk_embedding_tokens.push(token);
    }

    Ok(chunk_embedding_tokens)
}
