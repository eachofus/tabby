//! Модуль для обработки запросов автодополнения кода
//!
//! Предоставляет HTTP endpoint для генерации предложений автодополнения кода
//! на основе контекста. Поддерживает аутентификацию пользователей, ограничения
//! доступа к репозиториям и интеграцию с различными IDE через User-Agent.

// === IMPORTS ===
use std::sync::Arc;

use axum::{extract::State, Extension, Json};
use axum_extra::{headers, TypedHeader};
use hyper::StatusCode;
use tracing::{instrument, warn};

use tabby_common::axum::{AllowedCodeRepository, MaybeUser};

use crate::services::completion::{CompletionRequest, CompletionResponse, CompletionService};

// === FREE FUNCTIONS ===

/// Обрабатывает запросы автодополнения кода
///
/// Принимает контекст кода и возвращает предложения для автодополнения.
/// Поддерживает аутентификацию пользователей, проверку доступа к репозиториям
/// и отслеживание источника запроса через User-Agent для аналитики.
///
/// # Arguments
///
/// * `state` - Сервис автодополнения для генерации предложений
/// * `allowed_code_repository` - Ограничения доступа к репозиториям кода
/// * `user` - Опциональная информация о пользователе из заголовков аутентификации
/// * `user_agent` - Информация о клиенте (IDE/редактор) для аналитики
/// * `request` - Запрос автодополнения с контекстом кода
///
/// # Returns
///
/// Возвращает JSON с предложениями автодополнения или HTTP ошибку при неудаче.
///
/// # Errors
///
/// * `StatusCode::BAD_REQUEST` - При некорректном запросе или ошибках генерации
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:8080/v1/completions \
///   -H "Content-Type: application/json" \
///   -H "User-Agent: VSCode/1.0" \
///   -d '{
///     "language": "rust",
///     "segments": {
///       "prefix": "fn main() {",
///       "suffix": "}"
///     }
///   }'
/// ```
///
/// # Security
///
/// Функция проверяет права доступа к репозиториям через `AllowedCodeRepository`
/// и поддерживает аутентификацию через Bearer токены в заголовке Authorization.
#[utoipa::path(
    post,
    path = "/v1/completions",
    request_body = CompletionRequest,
    operation_id = "completion",
    tag = "v1",
    responses(
        (status = 200, description = "Success", body = CompletionResponse, content_type = "application/json"),
        (status = 400, description = "Bad Request")
    ),
    security(
        ("token" = [])
    )
)]
#[instrument(skip(state, request))]
pub async fn completions(
    State(state): State<Arc<CompletionService>>,
    Extension(allowed_code_repository): Extension<AllowedCodeRepository>,
    TypedHeader(MaybeUser(user)): TypedHeader<MaybeUser>,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    Json(mut request): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, StatusCode> {
    // Добавляем информацию о пользователе в запрос если она доступна
    if let Some(user) = user {
        request.user.replace(user);
    }

    // Извлекаем User-Agent для аналитики использования различных IDE
    let user_agent = user_agent.map(|x| x.0.to_string());

    // Генерируем автодополнение с учётом ограничений доступа
    match state
        .generate(&request, &allowed_code_repository, user_agent.as_deref())
        .await
    {
        Ok(resp) => Ok(Json(resp)),
        Err(err) => {
            // Логируем ошибку для диагностики, но не раскрываем детали клиенту
            warn!("Completion generation failed: {}", err);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}
