//! Модуль для обработки чат-запросов через OpenAI-совместимый API
//!
//! Предоставляет endpoints для взаимодействия с чат-моделями через
//! Server-Sent Events (SSE) streaming. Поддерживает аутентификацию,
//! логирование событий и обработку ошибок в соответствии с OpenAI API.

// === IMPORTS ===
use std::sync::Arc;

use async_openai_alt::error::OpenAIError;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use axum_extra::TypedHeader;
use futures::{Stream, StreamExt};
use hyper::StatusCode;
use tracing::{error, instrument, warn};

use tabby_common::{
    api::event::{Event as LoggerEvent, EventLogger},
    axum::MaybeUser,
};
use tabby_inference::ChatCompletionStream;

// === STRUCTS ===

/// Состояние для обработки чат-запросов
///
/// Содержит ссылки на сервис чат-дополнений и логгер событий.
/// Используется как shared state в Axum handlers для обеспечения
/// доступа к необходимым сервисам.
///
/// # Fields
///
/// * `chat_completion` - Сервис для генерации чат-ответов
/// * `logger` - Логгер для записи событий пользователей
pub struct ChatState {
    /// Сервис чат-дополнений для генерации ответов
    pub chat_completion: Arc<dyn ChatCompletionStream>,
    /// Логгер событий для аналитики и мониторинга
    pub logger: Arc<dyn EventLogger>,
}

// === FREE FUNCTIONS ===

/// OpenAPI документация для endpoint чат-дополнений (только для генерации схемы)
///
/// Эта функция используется исключительно для генерации OpenAPI документации
/// и не содержит реальной логики. Настоящая реализация находится в функции
/// `chat_completions`.
///
/// # Returns
///
/// Всегда возвращает `StatusCode::NOT_IMPLEMENTED` так как не предназначена
/// для выполнения.
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    operation_id = "chat_completions",
    tag = "v1",
    responses(
        (status = 200, description = "Success", content_type = "text/event-stream"),
        (status = 405, description = "When chat model is not specified, the endpoint returns 405 Method Not Allowed"),
        (status = 422, description = "When the prompt is malformed, the endpoint returns 422 Unprocessable Entity")
    ),
    security(
        ("token" = [])
    )
)]
#[allow(unused)]
pub async fn chat_completions_utoipa(_request: Json<serde_json::Value>) -> StatusCode {
    unimplemented!()
}

/// Обрабатывает запросы чат-дополнений с потоковой передачей ответа
///
/// Принимает запрос в формате OpenAI API и возвращает потоковый ответ
/// через Server-Sent Events. Поддерживает аутентификацию пользователей,
/// логирование событий и обработку ошибок потока.
///
/// # Arguments
///
/// * `state` - Состояние с сервисами чата и логирования
/// * `user` - Опциональная информация о пользователе из заголовков
/// * `request` - Запрос чат-дополнения в формате OpenAI API
///
/// # Returns
///
/// Возвращает SSE поток с чанками ответа или HTTP ошибку при неудаче.
///
/// # Errors
///
/// * `StatusCode::INTERNAL_SERVER_ERROR` - При ошибках инициализации потока
/// * Ошибки в потоке передаются как SSE события с информацией об ошибке
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:8080/v1/chat/completions \
///   -H "Content-Type: application/json" \
///   -d '{
///     "model": "gpt-3.5-turbo",
///     "messages": [{"role": "user", "content": "Hello!"}],
///     "stream": true
///   }'
/// ```
#[instrument(skip(state, request))]
pub async fn chat_completions(
    State(state): State<Arc<ChatState>>,
    TypedHeader(MaybeUser(user)): TypedHeader<MaybeUser>,
    Json(mut request): Json<async_openai_alt::types::CreateChatCompletionRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, anyhow::Error>>>, StatusCode> {
    // Добавляем информацию о пользователе в запрос если она доступна
    if let Some(user) = user {
        request.user.replace(user);
    }
    let user = request.user.clone();

    // Инициализируем поток чат-дополнений
    let s = match state.chat_completion.chat_stream(request).await {
        Ok(s) => s,
        Err(err) => {
            warn!("Error happens during chat completion: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Создаём асинхронный поток для SSE событий
    let s = async_stream::stream! {
        let mut s = s;
        while let Some(event) = s.next().await {
            match event {
                Ok(event) => {
                    // Успешно получен чанк ответа - отправляем как SSE событие
                    yield Ok(Event::default().json_data(event)?);
                }
                Err(err) => {
                    // Обрабатываем ошибки потока
                    if let OpenAIError::StreamError(content) = err {
                        if content == "Stream ended" {
                            // Нормальное завершение потока
                            break;
                        }
                    } else {
                        // Критическая ошибка - логируем и передаём клиенту
                        error!("Failed to get chat completion chunk: {:?}", err);
                        yield Err(err.into());
                    }
                }
            }
        }
    };

    // Логируем событие чат-дополнения для аналитики
    state.logger.log(user, LoggerEvent::ChatCompletion {});

    // Возвращаем SSE поток с keep-alive для поддержания соединения
    Ok(Sse::new(s).keep_alive(KeepAlive::default()))
}
