//! API для получения настроек сервера
//!
//! Предоставляет REST эндпоинт для получения конфигурации сервера,
//! включая настройки телеметрии и параметры поведения клиентов.

// === IMPORTS ===
use axum::Json;

use tabby_common::api::server_setting::ServerSetting;

// === FREE FUNCTIONS ===

/// Возвращает настройки сервера
///
/// Эндпоинт API для получения конфигурации сервера, включая настройки
/// телеметрии и другие параметры поведения клиентов. Используется
/// клиентскими приложениями для адаптации своего поведения под
/// конфигурацию сервера.
///
/// # Returns
///
/// JSON-ответ с настройками сервера, включая флаги для управления
/// телеметрией и другими клиентскими функциями.
///
/// # Examples
///
/// ```rust
/// // Вызов эндпоинта вернет настройки по умолчанию
/// let response = setting().await;
/// assert_eq!(response.disable_client_side_telemetry, false);
/// ```
#[utoipa::path(
    get,
    path = "/v1beta/server_setting",
    tag = "v1beta",
    operation_id = "config",
    responses(
        (status = 200, description = "Success", body = ServerSetting, content_type = "application/json"),
    ),
    security(
        ("token" = [])
    )
)]
pub async fn setting() -> Json<ServerSetting> {
    // Возвращаем настройки по умолчанию
    // TODO: В будущем настройки должны читаться из конфигурационного файла
    let config = ServerSetting {
        disable_client_side_telemetry: false,
    };
    Json(config)
}
