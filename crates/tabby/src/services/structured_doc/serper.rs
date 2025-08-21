//! Интеграция с Serper API для поиска в интернете
//!
//! Реализует поиск документов через внешний API Serper, который предоставляет
//! доступ к результатам поиска Google. Используется для получения актуальной
//! информации из интернета, когда локальные индексы не содержат нужных данных.

// === IMPORTS ===
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::warn;

use tabby_common::api::structured_doc::{
    DocSearch, DocSearchDocument, DocSearchError, DocSearchHit, DocSearchResponse,
    DocSearchWebDocument,
};

// === CONSTANTS ===
/// URL эндпоинта Serper API для поиска
const SERPER_SEARCH_URL: &str = "https://google.serper.dev/search";

/// Заголовок для передачи API ключа
const API_KEY_HEADER: &str = "X-API-KEY";

// === STRUCTS ===
/// Запрос к Serper API
///
/// Содержит параметры поискового запроса, включая текст запроса,
/// количество результатов и номер страницы.
#[derive(Debug, Serialize)]
struct SerperRequest {
    /// Текст поискового запроса
    q: String,
    /// Количество результатов на странице
    num: usize,
    /// Номер страницы (начиная с 0)
    page: usize,
}

/// Ответ от Serper API
///
/// Содержит массив органических результатов поиска из Google.
#[derive(Debug, Deserialize)]
struct SerperResponse {
    /// Список органических результатов поиска
    organic: Vec<SerperOrganicHit>,
}

/// Отдельный результат поиска от Serper
///
/// Представляет один элемент из органических результатов поиска,
/// содержащий заголовок, описание и ссылку на страницу.
#[derive(Debug, Deserialize)]
struct SerperOrganicHit {
    /// Заголовок страницы
    title: String,
    /// Краткое описание содержимого страницы
    snippet: String,
    /// URL ссылка на страницу
    link: String,
}

/// Сервис поиска через Serper API
///
/// Предоставляет возможность поиска документов в интернете через
/// API Serper, который использует результаты поиска Google.
/// Подходит для получения актуальной информации и документации.
pub struct SerperService {
    /// HTTP клиент с настроенными заголовками аутентификации
    client: reqwest::Client,
}

// === IMPLEMENTATIONS ===
impl SerperService {
    /// Создаёт новый экземпляр сервиса Serper
    ///
    /// Инициализирует HTTP клиент с API ключом для аутентификации
    /// в сервисе Serper. Ключ передаётся в заголовке X-API-KEY.
    ///
    /// # Arguments
    ///
    /// * `api_key` - API ключ для доступа к сервису Serper
    ///
    /// # Returns
    ///
    /// Новый экземпляр `SerperService`
    ///
    /// # Panics
    ///
    /// Паникует если:
    /// - API ключ содержит недопустимые символы для HTTP заголовка
    /// - Не удалось создать HTTP клиент
    ///
    /// # Examples
    ///
    /// ```rust
    /// let api_key = std::env::var("SERPER_API_KEY")
    ///     .expect("SERPER_API_KEY должен быть установлен");
    /// let service = SerperService::new(&api_key);
    /// ```
    pub fn new(api_key: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            API_KEY_HEADER,
            api_key
                .parse()
                .expect("Не удалось разобрать API ключ Serper"),
        );
        
        Self {
            client: reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .expect("Не удалось создать HTTP клиент"),
        }
    }
}

#[async_trait]
impl DocSearch for SerperService {
    /// Выполняет поиск документов через Serper API
    ///
    /// Отправляет запрос к API Serper для поиска информации в интернете.
    /// Преобразует результаты Google поиска в унифицированный формат
    /// для интеграции с остальной системой поиска.
    ///
    /// # Arguments
    ///
    /// * `source_ids` - Идентификаторы источников (игнорируются Serper API)
    /// * `q` - Текст поискового запроса
    /// * `limit` - Максимальное количество результатов
    ///
    /// # Returns
    ///
    /// `DocSearchResponse` с найденными документами или ошибка
    ///
    /// # Errors
    ///
    /// Возвращает `DocSearchError::Other` если:
    /// - Сетевая ошибка при запросе к API
    /// - Ошибка десериализации ответа
    /// - API вернул ошибку аутентификации или превышен лимит
    ///
    /// # Notes
    ///
    /// - Параметр `source_ids` игнорируется, так как Serper не поддерживает фильтрацию источников
    /// - Все результаты имеют score = 0.0, так как Serper не предоставляет оценки релевантности
    /// - Поиск всегда выполняется по первой странице (page = 0)
    async fn search(
        &self,
        source_ids: &[String],
        q: &str,
        limit: usize,
    ) -> Result<DocSearchResponse, DocSearchError> {
        // Предупреждаем о неподдерживаемой функциональности
        if !source_ids.is_empty() {
            warn!("Serper не поддерживает фильтрацию по источникам");
        }

        let request = SerperRequest {
            q: q.to_string(),
            num: limit,
            page: 0,
        };

        // Выполняем запрос к API Serper
        let response = self
            .client
            .post(SERPER_SEARCH_URL)
            .json(&request)
            .send()
            .await
            .map_err(|e| DocSearchError::Other(e.into()))?
            .json::<SerperResponse>()
            .await
            .map_err(|e| DocSearchError::Other(e.into()))?;

        // Преобразуем результаты в унифицированный формат
        let hits = response
            .organic
            .into_iter()
            .map(|hit| DocSearchHit {
                // Serper не предоставляет оценки релевантности
                score: 0.0,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: hit.title,
                    link: hit.link,
                    snippet: hit.snippet,
                }),
            })
            .collect();

        Ok(DocSearchResponse { hits })
    }
}
