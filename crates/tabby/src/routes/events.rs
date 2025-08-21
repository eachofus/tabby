//! Модуль для логирования пользовательских событий
//!
//! Предоставляет HTTP endpoint для записи событий взаимодействия пользователей
//! с системой автодополнения кода. Поддерживает различные типы событий:
//! просмотр предложений, выбор автодополнений и отклонение предложений.

// === IMPORTS ===
use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Query, State},
    Json,
};
use axum_extra::TypedHeader;
use hyper::StatusCode;

use tabby_common::{
    api::event::{Event, EventLogger, LogEventRequest, SelectKind},
    axum::MaybeUser,
};

// === FREE FUNCTIONS ===

/// Логирует события взаимодействия пользователя с автодополнениями
///
/// Принимает различные типы событий от клиентских приложений (IDE/редакторы)
/// и записывает их в систему аналитики для последующего анализа эффективности
/// автодополнений и поведения пользователей.
///
/// # Поддерживаемые типы событий
///
/// * `view` - Пользователь увидел предложение автодополнения
/// * `select` - Пользователь выбрал и применил автодополнение
/// * `dismiss` - Пользователь отклонил предложение автодополнения
///
/// # Arguments
///
/// * `logger` - Сервис логирования событий для записи в хранилище
/// * `user` - Опциональная информация о пользователе из заголовков аутентификации
/// * `params` - Query параметры запроса (например, `select_kind` для уточнения типа выбора)
/// * `request` - Данные события с идентификаторами и метриками
///
/// # Returns
///
/// Возвращает HTTP статус код:
/// * `200 OK` - Событие успешно записано
/// * `400 Bad Request` - Неизвестный тип события
///
/// # Examples
///
/// ```bash
/// # Логирование просмотра автодополнения
/// curl -X POST http://localhost:8080/v1/events \
///   -H "Content-Type: application/json" \
///   -d '{
///     "event_type": "view",
///     "completion_id": "comp_123",
///     "choice_index": 0,
///     "view_id": "view_456"
///   }'
///
/// # Логирование выбора автодополнения с указанием типа выбора
/// curl -X POST "http://localhost:8080/v1/events?select_kind=line" \
///   -H "Content-Type: application/json" \
///   -d '{
///     "event_type": "select",
///     "completion_id": "comp_123",
///     "choice_index": 0,
///     "view_id": "view_456",
///     "elapsed": 1500
///   }'
/// ```
///
/// # Event Types Details
///
/// ## View Event
/// Записывается когда пользователь видит предложение автодополнения.
/// Помогает измерить охват и частоту показов.
///
/// ## Select Event
/// Записывается когда пользователь принимает автодополнение.
/// Включает время принятия решения и тип выбора (полная строка или частичный).
///
/// ## Dismiss Event
/// Записывается когда пользователь явно отклоняет предложение.
/// Помогает понять причины отказов от автодополнений.
#[utoipa::path(
    post,
    path = "/v1/events",
    request_body = LogEventRequest,
    tag = "v1",
    operation_id = "event",
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Bad Request")
    ),
    security(
        ("token" = [])
    )
)]
pub async fn log_event(
    State(logger): State<Arc<dyn EventLogger>>,
    TypedHeader(MaybeUser(user)): TypedHeader<MaybeUser>,
    Query(params): Query<HashMap<String, String>>,
    Json(request): Json<LogEventRequest>,
) -> StatusCode {
    match request.event_type.as_str() {
        "view" => {
            // Логируем событие просмотра автодополнения
            logger.log(
                user,
                Event::View {
                    completion_id: request.completion_id,
                    choice_index: request.choice_index,
                    view_id: request.view_id,
                },
            );
            StatusCode::OK
        }
        "select" => {
            // Определяем тип выбора из query параметров
            // Параметр select_kind=line указывает на выбор полной строки
            let select_kind = if params
                .get("select_kind")
                .map(|kind| kind == "line")
                .unwrap_or(false)
            {
                Some(SelectKind::Line)
            } else {
                None
            };

            // Логируем событие выбора автодополнения с метриками времени
            logger.log(
                user,
                Event::Select {
                    completion_id: request.completion_id,
                    choice_index: request.choice_index,
                    kind: select_kind,
                    view_id: request.view_id,
                    elapsed: request.elapsed,
                },
            );
            StatusCode::OK
        }
        "dismiss" => {
            // Логируем событие отклонения автодополнения
            logger.log(
                user,
                Event::Dismiss {
                    completion_id: request.completion_id,
                    choice_index: request.choice_index,
                    view_id: request.view_id,
                    elapsed: request.elapsed,
                },
            );
            StatusCode::OK
        }
        _ => {
            // Неизвестный тип события - возвращаем ошибку
            StatusCode::BAD_REQUEST
        }
    }
}
