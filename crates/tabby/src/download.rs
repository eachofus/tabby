//! Модуль для загрузки моделей машинного обучения
//!
//! Предоставляет CLI интерфейс и функциональность для загрузки моделей
//! из удалённых репозиториев с поддержкой локального кеширования.

// === IMPORTS ===
use clap::Args;
use tracing::info;

use tabby_download::download_model;

// === STRUCTS ===

/// Аргументы командной строки для загрузки моделей
///
/// Содержит параметры для указания модели и настроек загрузки.
/// Используется CLI для парсинга аргументов команды download.
///
/// # Examples
///
/// ```rust
/// use clap::Parser;
/// 
/// let args = DownloadArgs {
///     model: "codellama:7b".to_string(),
///     prefer_local_file: false,
/// };
/// ```
#[derive(Args)]
pub struct DownloadArgs {
    /// Идентификатор модели для загрузки
    ///
    /// Указывает какую модель необходимо загрузить из репозитория.
    /// Формат зависит от конкретного провайдера моделей.
    #[clap(long)]
    model: String,

    /// Предпочитать локальные файлы вместо проверки удалённых версий
    ///
    /// Если установлено в true, пропускает проверку наличия обновлённой
    /// версии модели в удалённом репозитории и использует локальную копию.
    #[clap(long, default_value_t = false)]
    prefer_local_file: bool,
}

// === FREE FUNCTIONS ===

/// Выполняет загрузку модели согласно переданным аргументам
///
/// Основная функция для загрузки моделей машинного обучения.
/// Использует библиотеку tabby_download для фактической загрузки
/// и логирует результат операции.
///
/// # Arguments
///
/// * `args` - Аргументы загрузки, содержащие идентификатор модели и настройки
///
/// # Examples
///
/// ```rust
/// let args = DownloadArgs {
///     model: "codellama:7b".to_string(),
///     prefer_local_file: false,
/// };
/// main(&args).await;
/// ```
pub async fn main(args: &DownloadArgs) {
    download_model(&args.model, args.prefer_local_file, None).await;
    info!("model '{}' is ready", args.model);
}
