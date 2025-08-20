// === IMPORTS ===
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// === STRUCTS ===
/// Ответ от Katana сканера веб-приложений
///
/// Представляет полный результат сканирования одного HTTP запроса,
/// включая детали запроса, ответа и временную метку для аналитики.
/// Используется для анализа веб-технологий и API паттернов.
///
/// # Поля
///
/// * `timestamp` - Время выполнения запроса (для будущей аналитики)
/// * `request` - Детали HTTP запроса
/// * `response` - Детали HTTP ответа с извлеченными технологиями
#[derive(Deserialize, Debug)]
pub struct KatanaRequestResponse {
    #[allow(dead_code)] // TODO: Будет использоваться в расширенной аналитике
    pub timestamp: String,
    pub request: KatanaRequest,
    pub response: KatanaResponse,
}

/// HTTP запрос от Katana сканера
///
/// Содержит информацию о выполненном HTTP запросе, включая метод,
/// конечную точку и сырые данные для дальнейшего анализа паттернов API.
///
/// # Поля
///
/// * `method` - HTTP метод (GET, POST, etc.)
/// * `endpoint` - URL конечной точки
/// * `raw` - Сырые данные запроса для ML обработки
#[derive(Deserialize, Debug)]
pub struct KatanaRequest {
    #[allow(dead_code)] // TODO: Будет использоваться для анализа API паттернов
    pub method: String,
    pub endpoint: String,
    #[allow(dead_code)] // TODO: Будет использоваться для ML обработки
    pub raw: String,
}

/// HTTP ответ от Katana сканера
///
/// Содержит детали HTTP ответа включая заголовки, тело ответа
/// и автоматически извлеченные технологии веб-стека.
/// Основа для анализа технологического стека приложений.
///
/// # Поля
///
/// * `status_code` - HTTP статус код ответа
/// * `headers` - Заголовки HTTP ответа
/// * `body` - Тело ответа (если доступно)
/// * `technologies` - Обнаруженные технологии (ключевое поле для анализа)
/// * `raw` - Сырые данные ответа
#[derive(Deserialize, Debug)]
pub struct KatanaResponse {
    pub status_code: Option<u16>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[allow(dead_code)] // TODO: Основа анализа технологического стека
    pub technologies: Option<Vec<String>>,
    pub raw: Option<String>,
}

/// Метаданные извлеченные из веб-страницы
///
/// Содержит основную информацию о странице, извлеченную в процессе краулинга.
/// Используется для индексации и поиска по содержимому веб-документов.
///
/// # Поля
///
/// * `title` - Заголовок страницы (очищенный от специальных символов)
/// * `description` - Описание страницы из meta тегов
///
/// # Examples
///
/// ```rust
/// let metadata = CrawledMetadata {
///     title: Some("Документация API".to_string()),
///     description: Some("Полное руководство по использованию API".to_string()),
/// };
/// ```
#[derive(Serialize)]
pub struct CrawledMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Краулированный веб-документ
///
/// Представляет полностью обработанный веб-документ с извлеченным
/// содержимым в формате Markdown и метаданными. Готов для индексации
/// и дальнейшей обработки в системе поиска.
///
/// # Поля
///
/// * `url` - Исходный URL документа
/// * `markdown` - Содержимое страницы в формате Markdown
/// * `metadata` - Извлеченные метаданные страницы
///
/// # Examples
///
/// ```rust
/// let document = CrawledDocument::new(
///     "https://example.com/docs".to_string(),
///     "# Заголовок\n\nСодержимое документа...".to_string(),
///     metadata
/// );
/// ```
#[derive(Serialize)]
pub struct CrawledDocument {
    pub url: String,
    pub markdown: String,
    pub metadata: CrawledMetadata,
}

// === IMPLEMENTATIONS ===
impl From<readable_readability::Metadata> for CrawledMetadata {
    /// Преобразует метаданные из библиотеки readability в наш формат
    ///
    /// Выполняет очистку заголовка от ASCII специальных символов
    /// и извлекает описание. Приоритет отдается заголовку статьи
    /// над заголовком страницы для лучшего качества индексации.
    fn from(metadata: readable_readability::Metadata) -> Self {
        // Очищаем заголовок от ASCII специальных символов для лучшей индексации
        // Включаем символы разметки, пунктуации и управляющие символы
        let trim_title_chars = [
            '#', '$', '%', '&', '*', '+', ',', '/', ':', ';', '=', '?', '@', '[', ']', '^', '`',
            '{', '|', '}', '~', '\n', ' ',
        ];
        
        // Предпочитаем заголовок статьи заголовку страницы для лучшего качества
        let title = metadata
            .article_title
            .or(metadata.page_title)
            .map(|x| x.trim_matches(trim_title_chars).to_owned());
            
        Self {
            title,
            description: metadata.description,
        }
    }
}

impl CrawledDocument {
    /// Создает новый краулированный документ
    ///
    /// # Arguments
    ///
    /// * `url` - URL исходного документа
    /// * `markdown` - Содержимое в формате Markdown
    /// * `metadata` - Метаданные документа
    ///
    /// # Returns
    ///
    /// Новый экземпляр `CrawledDocument`
    pub fn new(url: String, markdown: String, metadata: CrawledMetadata) -> Self {
        Self {
            url,
            markdown,
            metadata,
        }
    }
}
