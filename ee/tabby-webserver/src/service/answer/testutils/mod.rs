// === IMPORTS ===
use std::sync::Arc;

use anyhow;
use async_openai_alt::{
    error::{ApiError, OpenAIError},
    types::{
        ChatChoice, ChatChoiceStream, ChatCompletionResponseMessage, ChatCompletionResponseStream,
        ChatCompletionStreamResponseDelta, CompletionUsage, CreateChatCompletionRequest,
        CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FinishReason, Role,
    },
};
use async_trait::async_trait;
use futures;
use juniper::ID;

use tabby_common::{
    api::{
        code::{
            CodeSearch, CodeSearchDocument, CodeSearchError, CodeSearchHit, CodeSearchParams,
            CodeSearchQuery, CodeSearchResponse, CodeSearchScores,
        },
        structured_doc::{
            DocSearch, DocSearchDocument, DocSearchError, DocSearchHit, DocSearchResponse,
            DocSearchWebDocument,
        },
    },
    config::AnswerConfig,
};
use tabby_db::DbConn;
use tabby_inference::ChatCompletionStream;
use tabby_schema::{
    context::{ContextInfo, ContextService},
    integration::IntegrationService,
    job::JobService,
    policy::AccessPolicy,
    repository::RepositoryService,
    Result,
};

use crate::{integration, job, repository};

// === STRUCTS ===
/// Fake implementation of ChatCompletionStream for testing purposes
pub struct FakeChatCompletionStream {
    pub return_error: bool,
}

/// Fake implementation of CodeSearch that returns successful results
pub struct FakeCodeSearch;

/// Fake implementation of CodeSearch that returns NotReady error
pub struct FakeCodeSearchFailNotReady;

/// Fake implementation of CodeSearch that returns generic error
pub struct FakeCodeSearchFail;

/// Fake implementation of DocSearch for testing purposes
pub struct FakeDocSearch;

/// Fake implementation of ContextService for testing purposes
pub struct FakeContextService;

// === IMPLEMENTATIONS ===
#[async_trait]
impl ChatCompletionStream for FakeChatCompletionStream {
    async fn chat(
        &self,
        _request: CreateChatCompletionRequest,
    ) -> Result<CreateChatCompletionResponse, OpenAIError> {
        if self.return_error {
            return Err(OpenAIError::ApiError(ApiError {
                message: "error".to_string(),
                code: None,
                param: None,
                r#type: None,
            }));
        }

        Ok(CreateChatCompletionResponse {
            id: "test-response".to_owned(),
            created: 0,
            model: "ChatTabby".to_owned(),
            object: "chat.completion".to_owned(),
            choices: vec![ChatChoice {
                index: 0,
                #[allow(deprecated)]
                message: ChatCompletionResponseMessage {
                    role: Role::Assistant,
                    content: Some(
                        "1. What is the main functionality of the provided code?\n\
                         1. How does the code snippet implement a web server?\n\
                         2. Can you explain how the Flask app works in this context?"
                            .to_string(),
                    ),
                    tool_calls: None,
                    function_call: None, // Deprecated field, required for struct compatibility but should use tool_calls instead
                    refusal: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            system_fingerprint: Some("seed".to_owned()),
            service_tier: None,
            usage: Some(CompletionUsage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
        })
    }

    async fn chat_stream(
        &self,
        _request: CreateChatCompletionRequest,
    ) -> Result<ChatCompletionResponseStream, OpenAIError> {
        let stream = futures::stream::iter(vec![
            Ok(CreateChatCompletionStreamResponse {
                id: "test-stream-response".to_owned(),
                created: 0,
                model: "ChatTabby".to_owned(),
                object: "chat.completion.chunk".to_owned(),
                choices: vec![ChatChoiceStream {
                    index: 0,
                    #[allow(deprecated)]
                    delta: ChatCompletionStreamResponseDelta {
                        role: Some(Role::Assistant),
                        content: Some("This is the first part of the response. ".to_string()),
                        tool_calls: None,
                        function_call: None, // Deprecated field, required for struct compatibility but should use tool_calls instead
                        refusal: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                system_fingerprint: Some("seed".to_owned()),
                service_tier: None,
                usage: Some(CompletionUsage {
                    prompt_tokens: 1,
                    completion_tokens: 2,
                    total_tokens: 3,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                }),
            }),
            Ok(CreateChatCompletionStreamResponse {
                id: "test-stream-response".to_owned(),
                created: 0,
                model: "ChatTabby".to_owned(),
                object: "chat.completion.chunk".to_owned(),
                choices: vec![ChatChoiceStream {
                    index: 0,
                    #[allow(deprecated)]
                    delta: ChatCompletionStreamResponseDelta {
                        role: None,
                        content: Some("This is the second part of the response.".to_string()),
                        tool_calls: None,
                        function_call: None, // Deprecated field, required for struct compatibility but should use tool_calls instead
                        refusal: None,
                    },
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                }],
                system_fingerprint: Some("seed".to_owned()),
                service_tier: None,
                usage: Some(CompletionUsage {
                    prompt_tokens: 1,
                    completion_tokens: 2,
                    total_tokens: 3,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                }),
            }),
        ]);

        Ok(Box::pin(stream) as ChatCompletionResponseStream)
    }
}

#[async_trait]
impl CodeSearch for FakeCodeSearch {
    async fn search_in_language(
        &self,
        _query: CodeSearchQuery,
        _params: CodeSearchParams,
    ) -> Result<CodeSearchResponse, CodeSearchError> {
        Ok(CodeSearchResponse {
            hits: vec![
                CodeSearchHit {
                    doc: CodeSearchDocument {
                        filepath: "src/lib.rs".to_string(),
                        body: "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".to_string(),
                        start_line: Some(1),
                        language: "rust".to_string(),
                        file_id: "1".to_string(),
                        chunk_id: "chunk1".to_string(),
                        git_url: "https://github.com/test/repo".to_string(),
                        commit: Some("commit".to_string()),
                    },
                    scores: CodeSearchScores {
                        bm25: 0.8,
                        embedding: 0.9,
                        rrf: 0.85,
                    },
                },
                CodeSearchHit {
                    doc: CodeSearchDocument {
                        filepath: "src/main.rs".to_string(),
                        body: "fn main() {\n    println!(\"Hello World\");\n}".to_string(),
                        start_line: Some(1),
                        language: "rust".to_string(),
                        file_id: "2".to_string(),
                        chunk_id: "chunk2".to_string(),
                        git_url: "https://github.com/test/repo".to_string(),
                        commit: Some("commit".to_string()),
                    },
                    scores: CodeSearchScores {
                        bm25: 0.7,
                        embedding: 0.8,
                        rrf: 0.75,
                    },
                },
            ],
        })
    }
}

#[async_trait]
impl CodeSearch for FakeCodeSearchFailNotReady {
    async fn search_in_language(
        &self,
        _query: CodeSearchQuery,
        _params: CodeSearchParams,
    ) -> Result<CodeSearchResponse, CodeSearchError> {
        Err(CodeSearchError::NotReady)
    }
}

#[async_trait]
impl CodeSearch for FakeCodeSearchFail {
    async fn search_in_language(
        &self,
        _query: CodeSearchQuery,
        _params: CodeSearchParams,
    ) -> Result<CodeSearchResponse, CodeSearchError> {
        Err(CodeSearchError::Other(anyhow::anyhow!("error")))
    }
}

#[async_trait]
impl DocSearch for FakeDocSearch {
    async fn search(
        &self,
        _source_ids: &[String],
        _q: &str,
        _limit: usize,
    ) -> Result<DocSearchResponse, DocSearchError> {
        let hits = vec![
            DocSearchHit {
                score: 1.0,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: "Document 1".to_string(),
                    link: "https://example.com/doc1".to_string(),
                    snippet: "Snippet for Document 1".to_string(),
                }),
            },
            DocSearchHit {
                score: 0.9,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: "Document 2".to_string(),
                    link: "https://example.com/doc2".to_string(),
                    snippet: "Snippet for Document 2".to_string(),
                }),
            },
            DocSearchHit {
                score: 0.8,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: "Document 3".to_string(),
                    link: "https://example.com/doc3".to_string(),
                    snippet: "Snippet for Document 3".to_string(),
                }),
            },
            DocSearchHit {
                score: 0.7,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: "Document 4".to_string(),
                    link: "https://example.com/doc4".to_string(),
                    snippet: "Snippet for Document 4".to_string(),
                }),
            },
            DocSearchHit {
                score: 0.6,
                doc: DocSearchDocument::Web(DocSearchWebDocument {
                    title: "Document 5".to_string(),
                    link: "https://example.com/doc5".to_string(),
                    snippet: "Snippet for Document 5".to_string(),
                }),
            },
        ];
        Ok(DocSearchResponse { hits })
    }
}

#[async_trait]
impl ContextService for FakeContextService {
    async fn read(&self, _policy: Option<&AccessPolicy>) -> Result<ContextInfo> {
        Ok(ContextInfo { sources: vec![] })
    }
}

// === FREE FUNCTIONS ===
/// Create default answer configuration for testing
pub fn make_answer_config() -> AnswerConfig {
    AnswerConfig {
        code_search_params: make_code_search_params(),
        presence_penalty: 0.1,
        system_prompt: AnswerConfig::default_system_prompt(),
    }
}

/// Create default code search parameters for testing
pub fn make_code_search_params() -> CodeSearchParams {
    CodeSearchParams {
        min_bm25_score: 0.5,
        min_embedding_score: 0.7,
        min_rrf_score: 0.3,
        num_to_return: 5,
        num_to_score: 10,
    }
}

/// Create repository service with all required dependencies for testing
pub async fn make_repository_service(db: DbConn) -> Result<Arc<dyn RepositoryService>> {
    let job_service: Arc<dyn JobService> = Arc::new(job::create(db.clone()).await);
    let integration_service: Arc<dyn IntegrationService> =
        Arc::new(integration::create(db.clone(), job_service.clone()));
    Ok(repository::create(
        db.clone(),
        integration_service.clone(),
        job_service.clone(),
    ))
}

/// Create access policy for testing
pub async fn make_policy() -> AccessPolicy {
    AccessPolicy::new(
        DbConn::new_in_memory().await.unwrap(),
        &ID::from("nihao".to_string()),
        false,
    )
}
