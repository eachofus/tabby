//! Модуль для экспорта метрик в формате Prometheus
//!
//! Предоставляет HTTP endpoint для сбора метрик производительности и мониторинга
//! системы Tabby. Метрики используются системами мониторинга (Prometheus, Grafana)
//! для отслеживания состояния сервиса, анализа производительности и настройки алертов.

// === IMPORTS ===
use std::sync::Arc;

use axum::extract::State;
use axum_prometheus::metrics_exporter_prometheus::PrometheusHandle;

// === FREE FUNCTIONS ===

/// Возвращает метрики системы в формате Prometheus
///
/// Предоставляет endpoint для сбора метрик производительности сервиса Tabby
/// в стандартном формате Prometheus. Включает метрики запросов, времени ответа,
/// использования ресурсов и состояния внутренних компонентов.
///
/// # Arguments
///
/// * `state` - Handle для экспорта метрик Prometheus, содержащий накопленные данные
///
/// # Returns
///
/// Возвращает строку с метриками в текстовом формате Prometheus, готовую для
/// парсинга системами мониторинга. Формат соответствует спецификации
/// [Prometheus exposition format](https://prometheus.io/docs/instrumenting/exposition_formats/).
///
/// # Metrics Categories
///
/// ## HTTP метрики
/// - Количество запросов по endpoints
/// - Время ответа (гистограммы)
/// - Коды статусов HTTP
/// - Размеры запросов и ответов
///
/// ## Метрики автодополнения
/// - Количество запросов автодополнения
/// - Время генерации предложений
/// - Качество предложений (принятые/отклоненные)
/// - Использование моделей
///
/// ## Системные метрики
/// - Использование памяти
/// - Загрузка CPU
/// - Количество активных соединений
/// - Статус внешних зависимостей
///
/// # Examples
///
/// ```bash
/// # Получение метрик для Prometheus
/// curl -X GET http://localhost:8080/metrics \
///   -H "Accept: text/plain"
/// ```
///
/// Пример вывода:
/// ```text
/// # HELP http_requests_total Total number of HTTP requests
/// # TYPE http_requests_total counter
/// http_requests_total{method="GET",endpoint="/v1/completions"} 1234
/// 
/// # HELP http_request_duration_seconds HTTP request duration
/// # TYPE http_request_duration_seconds histogram
/// http_request_duration_seconds_bucket{le="0.1"} 100
/// http_request_duration_seconds_bucket{le="0.5"} 200
/// http_request_duration_seconds_sum 45.2
/// http_request_duration_seconds_count 250
/// 
/// # HELP completion_requests_total Total completion requests
/// # TYPE completion_requests_total counter
/// completion_requests_total{model="codegen",status="success"} 856
/// ```
///
/// # Integration
///
/// Этот endpoint интегрируется с:
/// - **Prometheus** - автоматический сбор метрик по расписанию
/// - **Grafana** - визуализация метрик и создание дашбордов
/// - **AlertManager** - настройка уведомлений по пороговым значениям
/// - **Kubernetes** - ServiceMonitor для автоматического обнаружения
///
/// # Configuration
///
/// Для настройки сбора метрик в Prometheus добавьте job:
/// ```yaml
/// scrape_configs:
///   - job_name: 'tabby'
///     static_configs:
///       - targets: ['localhost:8080']
///     metrics_path: '/metrics'
///     scrape_interval: 15s
/// ```
///
/// # Performance
///
/// Endpoint оптимизирован для частых запросов от систем мониторинга.
/// Метрики кэшируются и обновляются инкрементально для минимизации
/// влияния на производительность основного сервиса.
pub async fn metrics(State(state): State<Arc<PrometheusHandle>>) -> String {
    // Рендерим все накопленные метрики в текстовом формате Prometheus
    // PrometheusHandle эффективно агрегирует данные из всех источников
    state.render()
}
