//! Главный исполняемый модуль Tabby AI Code Assistant
//!
//! Предоставляет CLI интерфейс для запуска сервера автодополнения кода
//! и загрузки моделей машинного обучения. Поддерживает различные устройства
//! для инференса (CPU, CUDA, ROCm, Metal, Vulkan).

// === MODULES ===
mod download;
mod otel;
mod routes;
mod serve;
mod services;

// === IMPORTS ===
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

use clap::{Parser, Subcommand};

use tabby_common::config::{Config, ModelConfig};

// === MACROS ===

/// Макрос для фатального завершения программы с логированием ошибки
///
/// Логирует ошибку через tracing и завершает программу с кодом выхода 1.
/// Поддерживает как простые строки, так и форматированные сообщения.
///
/// # Examples
///
/// ```rust
/// fatal!("Configuration file not found");
/// fatal!("Failed to connect to {}: {}", host, error);
/// ```
#[macro_export]
macro_rules! fatal {
    ($msg:expr) => {
        ({
            tracing::error!($msg);
            std::process::exit(1);
        })
    };

    ($fmt:expr, $($arg:tt)*) => {
        ({
            tracing::error!($fmt, $($arg)*);
            std::process::exit(1);
        })
    };
}

// === ENUMS ===

/// Поддерживаемые устройства для инференса моделей
///
/// Определяет доступные варианты аппаратного ускорения для выполнения
/// моделей машинного обучения. Каждый вариант оптимизирован для
/// конкретного типа оборудования.
#[derive(clap::ValueEnum, strum::Display, PartialEq, Clone)]
pub enum Device {
    /// Центральный процессор (универсальная совместимость)
    #[strum(serialize = "cpu")]
    Cpu,

    /// NVIDIA CUDA (GPU ускорение для карт NVIDIA)
    #[strum(serialize = "cuda")]
    Cuda,

    /// AMD ROCm (GPU ускорение для карт AMD)
    #[strum(serialize = "rocm")]
    Rocm,

    /// Apple Metal (GPU ускорение для устройств Apple)
    #[strum(serialize = "metal")]
    Metal,

    /// Vulkan API (кроссплатформенное GPU ускорение)
    #[strum(serialize = "vulkan")]
    Vulkan,
}

/// Доступные команды CLI
///
/// Основные операции, которые может выполнять приложение:
/// запуск сервера для IDE интеграций и загрузка моделей.
#[derive(Subcommand)]
pub enum Commands {
    /// Запускает API сервер для интеграций с IDE и редакторами
    Serve(serve::ServeArgs),

    /// Загружает языковую модель для последующего использования
    Download(download::DownloadArgs),
}

// === STRUCTS ===

/// Основные аргументы командной строки
///
/// Содержит глобальные настройки приложения и подкоманды.
/// Автоматически генерирует help и version информацию.
///
/// # Examples
///
/// ```bash
/// tabby serve --model codellama:7b --port 8080
/// tabby download --model codellama:7b
/// ```
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Подкоманда для выполнения
    #[command(subcommand)]
    command: Commands,

    /// Endpoint для Open Telemetry (скрытый параметр для отладки)
    ///
    /// Используется для отправки телеметрии в системы мониторинга.
    /// Параметр скрыт от обычных пользователей.
    #[clap(hide = true, long)]
    otlp_endpoint: Option<String>,
}

// === FREE FUNCTIONS ===

/// Точка входа в приложение
///
/// Инициализирует систему логирования, парсит аргументы командной строки,
/// создаёт необходимые директории и запускает соответствующую подкоманду.
/// Также настраивает права доступа к директории данных на Unix системах.
///
/// # Panics
///
/// Паникует если:
/// - Не удаётся установить color_eyre для обработки ошибок
/// - Не удаётся загрузить конфигурацию
/// - Не удаётся создать корневую директорию tabby
/// - Не удаётся установить права доступа (только Unix)
#[tokio::main]
async fn main() {
    color_eyre::install().expect("Must be able to install color_eyre");

    let cli = Cli::parse();
    let _guard = otel::init_tracing_subscriber(cli.otlp_endpoint);

    let config = Config::load().expect("Must be able to load config");
    let root = tabby_common::path::tabby_root();
    std::fs::create_dir_all(&root).expect("Must be able to create tabby root");
    
    // Устанавливаем безопасные права доступа только для владельца на Unix системах
    #[cfg(target_family = "unix")]
    {
        let mut permissions = std::fs::metadata(&root).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&root, permissions).unwrap();
    }

    match cli.command {
        Commands::Serve(ref args) => serve::main(&config, args).await,
        Commands::Download(ref args) => download::main(args).await,
    }
}

/// Создаёт локальную конфигурацию модели на основе параметров
///
/// Генерирует конфигурацию для локального запуска модели с учётом
/// выбранного устройства и переменных окружения. Автоматически
/// определяет количество GPU слоёв и настройки оптимизации.
///
/// # Arguments
///
/// * `model` - Идентификатор модели
/// * `parallelism` - Уровень параллелизма для обработки
/// * `device` - Целевое устройство для инференса
///
/// # Returns
///
/// Возвращает настроенную конфигурацию модели для локального использования.
///
/// # Environment Variables
///
/// * `LLAMA_CPP_N_GPU_LAYERS` - Количество слоёв для GPU (по умолчанию 9999)
/// * `LLAMA_CPP_FAST_ATTENTION` - Включение быстрого внимания (если установлена)
fn to_local_config(model: &str, parallelism: u8, device: &Device) -> ModelConfig {
    // Определяем количество GPU слоёв в зависимости от устройства
    let num_gpu_layers = if *device != Device::Cpu {
        std::env::var("LLAMA_CPP_N_GPU_LAYERS")
            .map(|s| s.parse::<u16>().ok())
            .ok()
            .flatten()
            .unwrap_or(9999)
    } else {
        0
    };
    
    // Включаем быстрое внимание если установлена соответствующая переменная
    // Работает только когда модель передана через CLI аргументы
    let enable_fast_attention = Some(std::env::var("LLAMA_CPP_FAST_ATTENTION").is_ok());

    ModelConfig::new_local(model, parallelism, num_gpu_layers, enable_fast_attention)
}
