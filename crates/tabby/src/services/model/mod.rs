//! Загрузка и управление моделями машинного обучения
//!
//! Модуль предоставляет функциональность для загрузки различных типов моделей:
//! эмбеддингов, генерации кода и чат-моделей. Поддерживает как локальные модели
//! (llama.cpp), так и удалённые HTTP API с автоматической загрузкой при необходимости.

// === MODULES ===
pub use llama_cpp_server::PromptInfo;

// === IMPORTS ===
use std::{fs, sync::Arc};

use tabby_common::config::ModelConfig;
use tabby_download::{download_model, ModelKind};
use tabby_inference::{ChatCompletionStream, CodeGeneration, CompletionStream, Embedding};
use tracing::info;

// === TYPE ALIASES ===
/// Результат загрузки моделей для генерации кода и чата
///
/// Кортеж содержит:
/// - Движок генерации кода (опционально)
/// - Поток автодополнения (опционально)  
/// - Поток чат-взаимодействий (опционально)
/// - Информация о промптах (опционально)
type CodeGenerationResult = (
    Option<Arc<CodeGeneration>>,
    Option<Arc<dyn CompletionStream>>,
    Option<Arc<dyn ChatCompletionStream>>,
    Option<PromptInfo>,
);

/// Результат загрузки базовых моделей автодополнения и чата
///
/// Кортеж содержит:
/// - Поток автодополнения (опционально)
/// - Информация о промптах (опционально)
/// - Поток чат-взаимодействий (опционально)
type CompletionChatResult = (
    Option<Arc<dyn CompletionStream>>,
    Option<PromptInfo>,
    Option<Arc<dyn ChatCompletionStream>>,
);

// === FREE FUNCTIONS ===
/// Загружает модель эмбеддингов
///
/// Создаёт и возвращает экземпляр модели эмбеддингов на основе конфигурации.
/// Поддерживает только локальные модели через llama.cpp.
///
/// # Arguments
///
/// * `config` - Конфигурация модели с путём к файлу и параметрами
///
/// # Returns
///
/// Arc-обёрнутый трейт-объект для работы с эмбеддингами
///
/// # Examples
///
/// ```rust
/// let config = ModelConfig::Local(LocalModelConfig {
///     model_path: "/path/to/embedding/model".to_string(),
///     ..Default::default()
/// });
/// let embedding = load_embedding(&config).await;
/// ```
pub async fn load_embedding(config: &ModelConfig) -> Arc<dyn Embedding> {
    llama_cpp_server::create_embedding(config).await
}

/// Загружает модели для генерации кода и чат-взаимодействий
///
/// Создаёт полный набор моделей для работы с кодом: движок генерации кода,
/// поток автодополнения, чат-модель и информацию о промптах. Автоматически
/// оптимизирует загрузку при использовании одной модели для обеих задач.
///
/// # Arguments
///
/// * `completion_model` - Конфигурация модели для автодополнения (опционально)
/// * `chat_model` - Конфигурация чат-модели (опционально)
///
/// # Returns
///
/// Кортеж с загруженными компонентами:
/// - `CodeGeneration` - Высокоуровневый API для генерации кода
/// - `CompletionStream` - Низкоуровневый поток автодополнения
/// - `ChatCompletionStream` - Поток для чат-взаимодействий
/// - `PromptInfo` - Шаблоны промптов для моделей
///
/// # Examples
///
/// ```rust
/// let (code_gen, completion, chat, prompts) = 
///     load_code_generation_and_chat(Some(completion_config), Some(chat_config)).await;
/// 
/// if let Some(code_generator) = code_gen {
///     let suggestions = code_generator.generate(&context).await;
/// }
/// ```
pub async fn load_code_generation_and_chat(
    completion_model: Option<ModelConfig>,
    chat_model: Option<ModelConfig>,
) -> CodeGenerationResult {
    let (engine, prompt_info, chat) =
        load_completion_and_chat(completion_model.clone(), chat_model).await;
    
    // Создаём высокоуровневый API генерации кода на основе движка автодополнения
    let code = engine
        .clone()
        .map(|engine| Arc::new(CodeGeneration::new(engine, completion_model)));
    
    (code, engine, chat, prompt_info)
}

/// Загружает базовые модели автодополнения и чата
///
/// Внутренняя функция для загрузки моделей с оптимизацией для случая,
/// когда одна модель используется для обеих задач. Поддерживает как
/// локальные модели (llama.cpp), так и удалённые HTTP API.
///
/// # Arguments
///
/// * `completion_model` - Конфигурация модели автодополнения
/// * `chat_model` - Конфигурация чат-модели
///
/// # Returns
///
/// Кортеж с базовыми компонентами моделей
///
/// # Implementation Notes
///
/// - При совпадении локальных моделей создаёт единый экземпляр
/// - Для HTTP моделей создаёт отдельные клиенты
/// - Автоматически извлекает шаблоны промптов из конфигурации
async fn load_completion_and_chat(
    completion_model: Option<ModelConfig>,
    chat_model: Option<ModelConfig>,
) -> CompletionChatResult {
    // Оптимизация: если обе модели локальные и одинаковые, создаём единый экземпляр
    if let (Some(ModelConfig::Local(completion)), Some(ModelConfig::Local(chat))) =
        (&completion_model, &chat_model)
    {
        let (completion, prompt, chat) =
            llama_cpp_server::create_completion_and_chat(completion, chat).await;
        return (Some(completion), Some(prompt), Some(chat));
    }

    // Загружаем модель автодополнения
    let (completion, prompt) = if let Some(completion_model) = completion_model {
        match completion_model {
            ModelConfig::Http(http) => {
                let engine = http_api_bindings::create(&http).await;
                let (prompt_template, chat_template) =
                    http_api_bindings::build_completion_prompt(&http);
                (
                    Some(engine),
                    Some(PromptInfo {
                        prompt_template,
                        chat_template,
                    }),
                )
            }
            ModelConfig::Local(llama) => {
                let (stream, prompt) = llama_cpp_server::create_completion(&llama).await;
                (Some(stream), Some(prompt))
            }
        }
    } else {
        (None, None)
    };

    // Загружаем чат-модель
    let chat = if let Some(chat_model) = chat_model {
        match chat_model {
            ModelConfig::Http(http) => Some(http_api_bindings::create_chat(&http).await),
            ModelConfig::Local(llama) => {
                Some(llama_cpp_server::create_chat_completion(&llama).await)
            }
        }
    } else {
        None
    };

    (completion, prompt, chat)
}

/// Загружает модель из локального пути или скачивает при необходимости
///
/// Проверяет существование модели по указанному пути. Если файл не найден,
/// автоматически запускает процесс загрузки из удалённого репозитория.
/// Поддерживает различные типы моделей через параметр `kind`.
///
/// # Arguments
///
/// * `model` - Путь к файлу модели или идентификатор для загрузки
/// * `kind` - Тип модели (код, чат, эмбеддинги)
///
/// # Examples
///
/// ```rust
/// // Загрузка модели автодополнения кода
/// download_model_if_needed("codellama-7b.gguf", ModelKind::Completion).await;
/// 
/// // Загрузка чат-модели
/// download_model_if_needed("llama-2-chat-7b.gguf", ModelKind::Chat).await;
/// ```
///
/// # Behavior
///
/// - Если файл существует: выводит сообщение о загрузке из локального пути
/// - Если файл не найден: запускает автоматическую загрузку с прогрессом
/// - Поддерживает прерывание загрузки и автоматическое возобновление
pub async fn download_model_if_needed(model: &str, kind: ModelKind) {
    if fs::metadata(model).is_ok() {
        info!("Загрузка модели из локального пути: {}", model);
    } else {
        info!("Модель не найдена локально, начинаю загрузку: {}", model);
        download_model(model, true, Some(kind)).await;
    }
}
