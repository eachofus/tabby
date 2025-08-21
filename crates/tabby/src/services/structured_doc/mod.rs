//! Сервисы поиска по структурированным документам
//!
//! Модуль предоставляет различные реализации поиска по документации и коду:
//! локальный поиск через Tantivy с эмбеддингами и внешний поиск через Serper API.
//! Поддерживает семантический поиск и интеграцию с различными источниками данных.

// === MODULES ===
mod serper;
mod tantivy;

// === IMPORTS ===
use std::sync::Arc;

use tabby_common::api::structured_doc::DocSearch;
use tabby_inference::Embedding;

use super::tantivy::IndexReaderProvider;

// === FREE FUNCTIONS ===
/// Создаёт сервис поиска по документам с использованием Tantivy
///
/// Создаёт локальный сервис поиска, который использует индекс Tantivy
/// для полнотекстового поиска в сочетании с моделью эмбеддингов для
/// семантического поиска. Обеспечивает высокую скорость поиска и
/// точность результатов для локальных документов.
///
/// # Arguments
///
/// * `embedding` - Модель эмбеддингов для семантического поиска
/// * `provider` - Провайдер для чтения индекса Tantivy
///
/// # Returns
///
/// Реализация трейта `DocSearch` на основе Tantivy
///
/// # Examples
///
/// ```rust
/// use std::sync::Arc;
/// use tabby_services::structured_doc;
/// 
/// let embedding_model = load_embedding_model().await;
/// let index_provider = create_index_provider("/path/to/index").await;
/// 
/// let search_service = structured_doc::create(embedding_model, index_provider);
/// let results = search_service.search("rust async programming", 10).await?;
/// ```
///
/// # Implementation Details
///
/// - Использует гибридный подход: полнотекстовый + семантический поиск
/// - Индекс Tantivy обеспечивает быстрый полнотекстовый поиск
/// - Эмбеддинги позволяют находить семантически похожие документы
/// - Результаты ранжируются по релевантности
pub fn create(embedding: Arc<dyn Embedding>, provider: Arc<IndexReaderProvider>) -> impl DocSearch {
    tantivy::DocSearchService::new(embedding, provider)
}

/// Создаёт сервис поиска через Serper API
///
/// Создаёт сервис поиска, который использует внешний API Serper
/// для поиска информации в интернете. Подходит для случаев, когда
/// нужно найти актуальную информацию, не доступную в локальных индексах.
///
/// # Arguments
///
/// * `api_key` - API ключ для доступа к сервису Serper
///
/// # Returns
///
/// Реализация трейта `DocSearch` на основе Serper API
///
/// # Examples
///
/// ```rust
/// use tabby_services::structured_doc;
/// 
/// let api_key = std::env::var("SERPER_API_KEY")
///     .expect("SERPER_API_KEY должен быть установлен");
/// 
/// let search_service = structured_doc::create_serper(&api_key);
/// let results = search_service.search("latest rust features 2024", 5).await?;
/// ```
///
/// # Notes
///
/// - Требует активное подключение к интернету
/// - Подвержен лимитам API (запросы в минуту, месячные квоты)
/// - Результаты могут изменяться со временем
/// - Подходит для поиска актуальной информации и документации
pub fn create_serper(api_key: &str) -> impl DocSearch {
    serper::SerperService::new(api_key)
}
