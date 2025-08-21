//! Сервис мониторинга состояния системы и моделей
//!
//! Предоставляет информацию о здоровье системы, включая статус моделей,
//! характеристики оборудования и версию сборки. Поддерживает мониторинг
//! как локальных, так и удаленных моделей с детализацией по устройствам.

// === IMPORTS ===
use std::env::consts::ARCH;

use anyhow::Result;
use nvml_wrapper::Nvml;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use utoipa::ToSchema;

use tabby_common::config::{ModelConfig, ModelConfigGroup};

use crate::Device;

// === ENUMS ===
/// Состояние здоровья модели
///
/// Различает локальные и удаленные модели для предоставления
/// соответствующей информации о конфигурации и ресурсах.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
enum ModelHealth {
    /// Удаленная модель, доступная через API
    #[serde(rename = "remote")]
    Remote(RemoteModelHealth),
    
    /// Локальная модель, работающая на сервере
    #[serde(rename = "local")]
    Local(LocalModelHealth),
}

// === STRUCTS ===
/// Общее состояние здоровья системы
///
/// Содержит полную информацию о состоянии сервера, включая модели,
/// оборудование и версию. Используется для API эндпоинта /health.
///
/// # Поля устаревшие (deprecated)
///
/// Поля `model`, `chat_model`, `chat_device`, `device`, `cuda_devices`
/// планируются к удалению в будущих версиях. Используйте поле `models`.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct HealthState {
    /// Название модели автодополнения (устаревшее)
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    
    /// Название чат-модели (устаревшее)
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_model: Option<String>,
    
    /// Устройство для чат-модели (устаревшее)
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_device: Option<String>,
    
    /// Основное устройство (устаревшее)
    device: String,
    
    /// Список CUDA устройств (устаревшее)
    cuda_devices: Vec<String>,

    /// Актуальная информация о состоянии моделей
    models: ModelsHealth,

    /// Архитектура процессора (x86_64, aarch64, etc.)
    arch: String,
    
    /// Информация о процессоре
    cpu_info: String,
    
    /// Количество CPU ядер
    cpu_count: usize,

    /// Информация о версии сборки
    version: Version,
    
    /// Включен ли веб-сервер
    webserver: Option<bool>,
}

/// Состояние здоровья всех моделей в системе
///
/// Группирует информацию о различных типах моделей:
/// автодополнение, чат и эмбеддинги.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct ModelsHealth {
    /// Модель автодополнения кода (опциональная)
    #[serde(skip_serializing_if = "Option::is_none")]
    completion: Option<ModelHealth>,

    /// Чат-модель (опциональная)
    #[serde(skip_serializing_if = "Option::is_none")]
    chat: Option<ModelHealth>,

    /// Модель эмбеддингов (обязательная)
    embedding: ModelHealth,
}

/// Информация о удаленной модели
///
/// Содержит данные о модели, доступной через внешний API.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct RemoteModelHealth {
    /// Тип провайдера (openai, anthropic, etc.)
    kind: String,
    
    /// Название модели (опционально)
    #[serde(skip_serializing_if = "Option::is_none")]
    model_name: Option<String>,
    
    /// URL эндпоинта API
    api_endpoint: String,
}

/// Информация о локальной модели
///
/// Содержит данные о модели, работающей на локальном сервере.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct LocalModelHealth {
    /// Идентификатор модели
    model_id: String,
    
    /// Устройство, на котором работает модель
    device: String,
    
    /// Список доступных CUDA устройств
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cuda_devices: Vec<String>,
}

/// Информация о версии сборки
///
/// Содержит метаданные о сборке, генерируемые во время компиляции.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct Version {
    /// Дата сборки
    build_date: String,
    
    /// Временная метка сборки
    build_timestamp: String,
    
    /// SHA коммита Git
    git_sha: String,
    
    /// Описание Git (тег + коммиты)
    git_describe: String,
}

// === IMPLEMENTATIONS ===
impl From<&ModelConfig> for ModelHealth {
    /// Преобразует конфигурацию модели в информацию о здоровье
    ///
    /// Создает соответствующий вариант ModelHealth на основе типа конфигурации.
    /// Для локальных моделей поля device и cuda_devices заполняются позже.
    fn from(model_config: &ModelConfig) -> Self {
        match model_config {
            ModelConfig::Http(http) => ModelHealth::Remote(RemoteModelHealth {
                kind: http.kind.clone(),
                model_name: http.model_name.clone(),
                api_endpoint: http.api_endpoint.clone().unwrap_or_default(),
            }),
            ModelConfig::Local(llama) => ModelHealth::Local(LocalModelHealth {
                model_id: llama.model_id.clone(),
                device: String::new(), // Заполняется позже
                cuda_devices: vec![], // Заполняется позже
            }),
        }
    }
}

impl From<&ModelConfigGroup> for ModelsHealth {
    /// Преобразует группу конфигураций в информацию о здоровье моделей
    ///
    /// Создает ModelsHealth из всех доступных моделей в конфигурации.
    /// Модель эмбеддингов обязательна, остальные опциональны.
    fn from(model_config: &ModelConfigGroup) -> Self {
        let completion = model_config.completion.as_ref().map(ModelHealth::from);
        let chat = model_config.chat.as_ref().map(ModelHealth::from);
        let embedding = ModelHealth::from(&model_config.embedding);

        Self {
            completion,
            chat,
            embedding,
        }
    }
}

impl HealthState {
    /// Создает новое состояние здоровья системы
    ///
    /// Собирает информацию о моделях, оборудовании и системе для формирования
    /// полного отчета о состоянии сервера.
    ///
    /// # Arguments
    ///
    /// * `model_config` - Конфигурация всех моделей
    /// * `device` - Основное устройство для выполнения
    /// * `chat_device` - Устройство для чат-модели (опционально)
    /// * `webserver` - Статус веб-сервера (опционально)
    ///
    /// # Returns
    ///
    /// Новый экземпляр HealthState с актуальной информацией о системе
    pub fn new(
        model_config: &ModelConfigGroup,
        device: &Device,
        chat_device: Option<&Device>,
        webserver: Option<bool>,
    ) -> Self {
        let (cpu_info, cpu_count) = read_cpu_info();
        let cuda_devices = read_cuda_devices().unwrap_or_default();
        
        // Создаем базовую информацию о моделях
        let mut models = ModelsHealth::from(model_config);
        
        // Заполняем информацию об устройствах для локальных моделей
        if let Some(model) = &mut models.completion {
            if let ModelHealth::Local(ref mut local) = model {
                local.device = device.to_string();
                local.cuda_devices = cuda_devices.clone();
            }
        }
        
        if let Some(model) = &mut models.chat {
            if let ModelHealth::Local(ref mut local) = model {
                local.device = chat_device.unwrap_or(device).to_string();
                local.cuda_devices = cuda_devices.clone();
            }
        }
        
        if let ModelHealth::Local(ref mut local) = models.embedding {
            local.device = device.to_string();
            local.cuda_devices = cuda_devices.clone();
        }

        Self {
            // Устаревшие поля для обратной совместимости
            model: to_model_name(&model_config.completion),
            chat_model: to_model_name(&model_config.chat),
            chat_device: chat_device.map(|x| x.to_string()),
            device: device.to_string(),
            cuda_devices,
            
            // Актуальные данные
            models,
            arch: ARCH.to_string(),
            cpu_info,
            cpu_count,
            version: Version::new(),
            webserver,
        }
    }
}

impl Version {
    /// Создает информацию о версии из переменных сборки
    ///
    /// Использует переменные окружения, установленные vergen во время компиляции
    /// для получения актуальной информации о версии.
    fn new() -> Self {
        Self {
            build_date: env!("VERGEN_BUILD_DATE").to_string(),
            build_timestamp: env!("VERGEN_BUILD_TIMESTAMP").to_string(),
            git_sha: env!("VERGEN_GIT_SHA").to_string(),
            git_describe: env!("VERGEN_GIT_DESCRIBE").to_string(),
        }
    }
}

// === FREE FUNCTIONS ===
/// Извлекает название модели из конфигурации
///
/// Возвращает человекочитаемое название модели для отображения в API.
/// Для HTTP моделей использует model_name или "Remote" по умолчанию.
///
/// # Arguments
///
/// * `model` - Опциональная конфигурация модели
///
/// # Returns
///
/// Название модели или None если модель не настроена
fn to_model_name(model: &Option<ModelConfig>) -> Option<String> {
    if let Some(model) = model {
        match model {
            ModelConfig::Http(http) => http
                .model_name
                .clone()
                .or_else(|| Some("Remote".to_string())),
            ModelConfig::Local(llama) => Some(llama.model_id.clone()),
        }
    } else {
        None
    }
}

/// Читает информацию о процессоре системы
///
/// Получает бренд процессора и количество ядер с помощью библиотеки sysinfo.
/// Используется для мониторинга ресурсов системы.
///
/// # Returns
///
/// Кортеж (информация_о_процессоре, количество_ядер)
pub fn read_cpu_info() -> (String, usize) {
    let mut system = System::new_all();
    system.refresh_cpu_all();
    let cpus = system.cpus();
    let count = cpus.len();
    
    let info = if count > 0 {
        let cpu = &cpus[0];
        cpu.brand().to_string()
    } else {
        "unknown".to_string()
    };

    (info, count)
}

/// Читает список доступных CUDA устройств
///
/// Использует NVML для получения информации о GPU. В случае ошибки
/// (например, на macOS или в Docker без --gpus) возвращает пустой список.
///
/// # Returns
///
/// Список названий CUDA устройств
///
/// # Errors
///
/// Возвращает ошибку если NVML недоступен или произошла ошибка при чтении устройств
pub fn read_cuda_devices() -> Result<Vec<String>> {
    // В случае macOS или Docker контейнеров без --gpus флага,
    // Nvml::init() вернет ошибку. В таких сценариях мы
    // устанавливаем cuda_devices как пустой список, указывая что текущая
    // среда выполнения не поддерживает CUDA интерфейс.
    let nvml = Nvml::init()?;
    let mut cuda_devices = vec![];
    let device_count = nvml.device_count()?;
    
    for i in 0..device_count {
        let name = nvml.device_by_index(i)?.name()?;
        cuda_devices.push(name);
    }
    
    Ok(cuda_devices)
}
