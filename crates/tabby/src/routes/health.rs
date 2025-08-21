//! Модуль для проверки состояния здоровья системы
//!
//! Предоставляет HTTP endpoint для мониторинга состояния сервиса Tabby.
//! Используется системами мониторинга, балансировщиками нагрузки и
//! инструментами оркестрации для определения готовности сервиса к работе.

// === IMPORTS ===
use std::sync::Arc;

use axum::{extract::State, Json};

use crate::services::health::HealthState;

// === FREE FUNCTIONS ===

/// Возвращает текущее состояние здоровья системы
///
/// Предоставляет информацию о готовности сервиса к обработке запросов.
/// Endpoint используется для health checks в контейнерных окружениях,
/// мониторинга доступности сервиса и автоматического управления трафиком.
///
/// # Arguments
///
/// * `state` - Общее состояние здоровья системы, включающее статус всех компонентов
///
/// # Returns
///
/// Возвращает JSON с детальной информацией о состоянии системы:
/// - Статус готовности сервиса
/// - Состояние подключенных моделей
/// - Статус внешних зависимостей
/// - Метрики производительности
///
/// # Examples
///
/// ```bash
/// # Проверка состояния системы
/// curl -X GET http://localhost:8080/v1/health \
///   -H "Accept: application/json"
/// ```
///
/// Пример ответа:
/// ```json
/// {
///   "status": "ready",
///   "models": {
///     "completion_model": "loaded",
///     "embedding_model": "loading"
///   },
///   "dependencies": {
///     "database": "healthy",
///     "vector_store": "healthy"
///   },
///   "uptime_seconds": 3600
/// }
/// ```
///
/// # Health Check Integration
///
/// Этот endpoint интегрируется с:
/// - Kubernetes liveness/readiness probes
/// - Docker HEALTHCHECK инструкциями
/// - Load balancer health checks
/// - Системами мониторинга (Prometheus, Grafana)
///
/// # Response Codes
///
/// Всегда возвращает HTTP 200 OK с детальной информацией в JSON.
/// Клиенты должны анализировать содержимое ответа для определения
/// фактического состояния компонентов системы.
#[utoipa::path(
    get,
    path = "/v1/health",
    tag = "v1",
    responses(
        (status = 200, description = "Success", body = HealthState, content_type = "application/json"),
    ),
    security(
        ("token" = [])
    )
)]
pub async fn health(State(state): State<Arc<HealthState>>) -> Json<HealthState> {
    // Возвращаем клонированное состояние для избежания блокировок
    // Arc обеспечивает эффективное разделение данных между запросами
    Json(state.as_ref().clone())
}
