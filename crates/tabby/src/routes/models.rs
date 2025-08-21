//! API для получения информации о доступных моделях
//!
//! Предоставляет REST эндпоинт для получения списка поддерживаемых моделей
//! для автодополнения кода и чат-взаимодействий с OpenAPI документацией.

// === IMPORTS ===
use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use tabby_common::api::server_setting::ServerSetting;

// === STRUCTS ===

/// Информация о доступных моделях для различных типов задач
///
/// Содержит списки моделей, поддерживаемых для автодополнения кода
/// и чат-взаимодействий. Используется для предоставления клиентам
/// информации о доступных возможностях сервера.
///
/// # Examples
///
/// ```rust
/// let models = ModelInfo {
///     completion: Some(vec!["codellama-7b".to_string()]),
///     chat: Some(vec!["llama2-chat".to_string()]),
/// };
/// ```
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct ModelInfo {
    /// Список моделей, доступных для автодополнения кода
    completion: Option<Vec<String>>,
    /// Список моделей, доступных для чат-взаимодействий
    chat: Option<Vec<String>>,
}

// === IMPLEMENTATIONS ===

impl From<tabby_common::config::Config> for ModelInfo {
    /// Создает ModelInfo из конфигурации сервера
    ///
    /// Извлекает информацию о поддерживаемых моделях из HTTP-конфигурации
    /// для задач автодополнения и чата. Если модели не настроены или
    /// используется не HTTP-конфигурация, соответствующие поля будут None.
    fn from(value: tabby_common::config::Config) -> Self {
        let models = value.model;
        let mut http_model_configs = ModelInfo {
            completion: None,
            chat: None,
        };

        // Извлекаем модели для автодополнения из HTTP-конфигурации
        if let Some(tabby_common::config::ModelConfig::Http(completion_http_config)) =
            models.completion
        {
            if let Some(models) = completion_http_config.supported_models {
                http_model_configs.completion = Some(models.clone());
            }
        }

        // Извлекаем модели для чата из HTTP-конфигурации
        if let Some(tabby_common::config::ModelConfig::Http(chat_http_config)) = models.chat {
            if let Some(models) = chat_http_config.supported_models {
                http_model_configs.chat = Some(models.clone());
            }
        }

        http_model_configs
    }
}

// === FREE FUNCTIONS ===

/// Возвращает информацию о доступных моделях
///
/// Эндпоинт API для получения списка поддерживаемых моделей
/// для автодополнения кода и чат-взаимодействий. Используется
/// клиентами для определения доступных возможностей сервера.
///
/// # Returns
///
/// JSON-ответ с информацией о моделях, включая списки доступных
/// моделей для каждого типа задач.
#[utoipa::path(
    get,
    path = "/v1beta/models",
    tag = "v1beta",
    operation_id = "config",
    responses(
        (status = 200, description = "Success", body = ServerSetting, content_type = "application/json"),
    ),
    security(
        ("token" = [])
    )
)]
pub async fn models(State(state): State<Arc<ModelInfo>>) -> Json<ModelInfo> {
    Json(state.as_ref().clone())
}
