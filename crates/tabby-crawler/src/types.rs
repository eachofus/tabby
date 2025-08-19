// === IMPORTS ===
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// === STRUCTS ===
/// Response structure from Katana crawler containing request/response data
#[derive(Deserialize, Debug)]
pub struct KatanaRequestResponse {
    #[allow(dead_code)] // Field used for deserialization but not accessed directly in code
    pub timestamp: String,
    pub request: KatanaRequest,
    pub response: KatanaResponse,
}

/// HTTP request information captured by Katana crawler
#[derive(Deserialize, Debug)]
pub struct KatanaRequest {
    #[allow(dead_code)] // Field used for deserialization but not accessed directly in code
    pub method: String,
    pub endpoint: String,
    #[allow(dead_code)] // Field used for deserialization but not accessed directly in code
    pub raw: String,
}

/// HTTP response information captured by Katana crawler
#[derive(Deserialize, Debug)]
pub struct KatanaResponse {
    pub status_code: Option<u16>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[allow(dead_code)] // Field used for deserialization but not accessed directly in code
    pub technologies: Option<Vec<String>>,
    pub raw: Option<String>,
}

/// Metadata extracted from crawled web pages
#[derive(Serialize)]
pub struct CrawledMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Complete crawled document with content and metadata
#[derive(Serialize)]
pub struct CrawledDocument {
    pub url: String,
    pub markdown: String,
    pub metadata: CrawledMetadata,
}

// === IMPLEMENTATIONS ===
impl From<readable_readability::Metadata> for CrawledMetadata {
    fn from(metadata: readable_readability::Metadata) -> Self {
        // Trim all ascii special chars from title
        let trim_title_chars = [
            '#', '$', '%', '&', '*', '+', ',', '/', ':', ';', '=', '?', '@', '[', ']', '^', '`',
            '{', '|', '}', '~', '\n', ' ',
        ];
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
    /// Create a new crawled document with URL, markdown content, and metadata
    pub fn new(url: String, markdown: String, metadata: CrawledMetadata) -> Self {
        Self {
            url,
            markdown,
            metadata,
        }
    }
}
