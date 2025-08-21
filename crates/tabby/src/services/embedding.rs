//! Сервис создания embedding моделей
//!
//! Предоставляет фабричную функцию для создания экземпляров embedding моделей
//! на основе конфигурации. Использует централизованную загрузку через model модуль.

// === IMPORTS ===
use std::sync::Arc;

use tabby_common::config::ModelConfig;
use tabby_inference::Embedding;

use super::model;

// === FREE FUNCTIONS ===
/// Создает экземпляр embedding модели
///
/// Загружает и инициализирует embedding модель на основе переданной конфигурации.
/// Использует централизованную систему загрузки моделей для обеспечения
/// единообразного управления ресурсами и кэширования.
///
/// # Arguments
///
/// * `config` - Конфигурация модели, содержащая путь к файлам и параметры загрузки
///
/// # Returns
///
/// Возвращает `Arc<dyn Embedding>` - потокобезопасный экземпляр embedding модели,
/// готовый для использования в многопоточном окружении.
///
/// # Examples
///
/// ```rust
/// use tabby_common::config::ModelConfig;
/// 
/// let config = ModelConfig::new("path/to/embedding/model");
/// let embedding = create(&config).await;
/// let vector = embedding.embed("sample text").await?;
/// ```
pub async fn create(config: &ModelConfig) -> Arc<dyn Embedding> {
    model::load_embedding(config).await
}
