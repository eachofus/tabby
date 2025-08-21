//! Модуль для настройки OpenTelemetry и системы трассировки
//!
//! Предоставляет функциональность для инициализации распределённой трассировки
//! с поддержкой экспорта телеметрии в OTLP-совместимые системы мониторинга.
//! Интегрируется с tracing-subscriber для единого логирования.

// === IMPORTS ===
use std::time::Duration;

use opentelemetry::{trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::{
    attribute::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_NAME, SERVICE_VERSION},
    SCHEMA_URL,
};
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry,
};

// === STRUCTS ===

/// RAII guard для корректного завершения OpenTelemetry провайдера
///
/// Обеспечивает автоматическое завершение работы tracer provider
/// при выходе из области видимости, гарантируя отправку всех
/// накопленных данных телеметрии.
///
/// # Examples
///
/// ```rust
/// let _guard = init_tracing_subscriber(Some("http://localhost:4317".to_string()));
/// // При выходе из области видимости guard автоматически завершит tracer provider
/// ```
pub struct OtelGuard {
    /// Провайдер трассировки, который нужно корректно завершить
    tracer_provider: Option<TracerProvider>,
}

// === IMPLEMENTATIONS ===

impl Drop for OtelGuard {
    /// Корректно завершает работу tracer provider при уничтожении guard
    ///
    /// Вызывает shutdown() для провайдера трассировки, что гарантирует
    /// отправку всех буферизованных span'ов в систему мониторинга.
    /// Ошибки завершения выводятся в stderr.
    fn drop(&mut self) {
        if let Some(tracer_provider) = self.tracer_provider.take() {
            if let Err(err) = tracer_provider.shutdown() {
                eprintln!("{err:?}");
            }
        }
    }
}

// === FREE FUNCTIONS ===

/// Создаёт ресурс OpenTelemetry с метаданными сервиса
///
/// Формирует описание сервиса для телеметрии, включая имя, версию
/// и среду развёртывания. Использует информацию из Cargo.toml
/// для автоматического определения имени и версии пакета.
///
/// # Returns
///
/// Возвращает настроенный ресурс с семантическими атрибутами
/// согласно спецификации OpenTelemetry.
fn resource() -> Resource {
    Resource::from_schema_url(
        [
            KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, "develop"),
        ],
        SCHEMA_URL,
    )
}

/// Инициализирует провайдер трассировки для экспорта в OTLP
///
/// Создаёт и настраивает TracerProvider с OTLP экспортером для отправки
/// данных трассировки в совместимые системы мониторинга (Jaeger, Zipkin и др.).
/// Использует полную выборку (100%) и случайную генерацию ID.
///
/// # Arguments
///
/// * `otlp_endpoint` - URL endpoint для отправки телеметрии (например, "http://localhost:4317")
///
/// # Returns
///
/// Возвращает настроенный TracerProvider готовый к использованию.
///
/// # Panics
///
/// Паникует если не удаётся создать OTLP экспортер с указанным endpoint.
fn init_tracer_provider(otlp_endpoint: String) -> TracerProvider {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    TracerProvider::builder()
        // Используем полную выборку для разработки (100% span'ов)
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            1.0,
        ))))
        // Для AWS X-Ray можно использовать XrayIdGenerator
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource())
        .with_batch_exporter(exporter, runtime::Tokio)
        .build()
}

/// Инициализирует систему трассировки с поддержкой OpenTelemetry
///
/// Настраивает tracing-subscriber с комбинацией слоёв для логирования
/// и телеметрии. Поддерживает как локальное логирование, так и экспорт
/// в внешние системы мониторинга через OTLP.
///
/// # Arguments
///
/// * `otlp_endpoint` - Опциональный URL для экспорта телеметрии.
///                     Если None, используется только локальное логирование.
///
/// # Returns
///
/// Возвращает OtelGuard для корректного завершения работы OpenTelemetry.
///
/// # Environment Variables
///
/// * `RUST_LOG` - Переопределяет уровни логирования по умолчанию
///
/// # Examples
///
/// ```rust
/// // Только локальное логирование
/// let _guard = init_tracing_subscriber(None);
///
/// // С экспортом в Jaeger
/// let _guard = init_tracing_subscriber(Some("http://localhost:4317".to_string()));
/// ```
pub fn init_tracing_subscriber(otlp_endpoint: Option<String>) -> OtelGuard {
    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();

    // Настраиваем OpenTelemetry слой если указан endpoint
    let tracer_provider = if let Some(endpoint) = otlp_endpoint {
        let tracer_provider = init_tracer_provider(endpoint);
        let tracer = tracer_provider.tracer("tracing-otel-subscriber");
        layers.push(Box::new(OpenTelemetryLayer::new(tracer)));
        Some(tracer_provider)
    } else {
        None
    };

    // Добавляем слой форматированного вывода с информацией о файле и строке
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .boxed();
    layers.push(fmt_layer);

    // Настраиваем уровни логирования в зависимости от режима сборки
    let mut dirs = if cfg!(feature = "prod") {
        "tabby=info,otel=debug,http_api_bindings=info,llama_cpp_server=info".into()
    } else {
        "tabby=debug,otel=debug,http_api_bindings=debug,llama_cpp_server=debug".into()
    };

    // Добавляем пользовательские настройки из переменной окружения
    if let Ok(env) = std::env::var(EnvFilter::DEFAULT_ENV) {
        dirs = format!("{dirs},{env}")
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .parse_lossy(dirs);

    tracing_subscriber::registry()
        .with(layers)
        .with(env_filter)
        .init();

    OtelGuard { tracer_provider }
}
