// === IMPORTS ===
use std::sync::Arc;

use anyhow::Result;
use async_stream::stream;
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde_json::json;
use tabby_common::index::structured_doc::fields;
use tabby_inference::Embedding;
use tokio::task::JoinHandle;

use super::{build_tokens, BuildStructuredDoc};

// === STRUCTS ===
/// Документ pull request для индексации
///
/// Представляет pull request или merge request из системы контроля версий
/// (GitHub, GitLab и др.). Содержит метаданные запроса на слияние и опционально
/// diff изменений для анализа кода.
///
/// # Fields
///
/// * `link` - Ссылка на pull request
/// * `title` - Заголовок запроса
/// * `author_email` - Email автора (опционально)
/// * `body` - Описание изменений
/// * `diff` - Diff изменений (до 1MB, опционально)
/// * `merged` - Статус слияния запроса
pub struct PullDocument {
    pub link: String,
    pub title: String,
    pub author_email: Option<String>,
    pub body: String,

    /// Diff представляет изменения кода в данном PR,
    /// включая метаданные, затронутые диапазоны строк и добавленные (+) или удаленные (-) строки.
    /// Подробности формата diff см. в документации:
    /// https://git-scm.com/docs/diff-format\#_combined_diff_format
    ///
    /// Diff сохраняется только если его размер не превышает 1MB
    pub diff: Option<String>,
    pub merged: bool,
}

// === IMPLEMENTATIONS ===
#[async_trait]
impl<'content_chunks> BuildStructuredDoc<'content_chunks> for PullDocument {
    fn should_skip(&self) -> bool {
        // Никогда не пропускаем pull request - они всегда важны для индексации
        false
    }

    async fn build_attributes(&self) -> serde_json::Value {
        json!({
            fields::pull::LINK: self.link,
            fields::pull::TITLE: self.title,
            fields::pull::AUTHOR_EMAIL: self.author_email,
            fields::pull::BODY: self.body,
            fields::pull::DIFF: self.diff,
            fields::pull::MERGED: self.merged,
        })
    }

    async fn build_chunk_attributes(
        &self,
        embedding: Arc<dyn Embedding>,
    ) -> BoxStream<'content_chunks, JoinHandle<Result<(Vec<String>, serde_json::Value)>>> {
        // В настоящее время не индексируем diff - только заголовок и описание
        // TODO: Рассмотреть возможность индексации diff для улучшения поиска по коду
        let text = format!("{}\n\n{}", self.title, self.body);
        
        let s = stream! {
            yield tokio::spawn(async move {
                let tokens = match build_tokens(embedding, &text).await {
                    Ok(tokens) => tokens,
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to build tokens for text: {}", e));
                    }
                };
                let chunk_attributes = json!({});
                Ok((tokens, chunk_attributes))
            });
        };

        Box::pin(s)
    }
}
