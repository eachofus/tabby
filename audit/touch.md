# Техническая спецификация организации Rust кода

## Версия: 1.0

## Дата: 17 августа 2025

---

## 1. Общие принципы

### 1.1 Цель

Обеспечить единообразную, читаемую и поддерживаемую структуру файлов в Rust проектах.

### 1.2 Область применения

- Все `.rs` файлы в проекте
- Библиотечный и прикладной код
- Тесты и примеры

---

## 2. Структура файла

### 2.1 Обязательный порядок секций

```rust
// === MODULES ===
// Объявления модулей и re-exports

// === IMPORTS ===  
// Use statements (внешние и внутренние)

// === CONSTANTS ===
// Константы и статические переменные

// === TYPE ALIASES ===
// Псевдонимы типов

// === MACROS ===
// Определения макросов

// === ENUMS ===
// Перечисления

// === STRUCTS ===
// Структуры данных

// === TRAITS ===
// Определения трейтов

// === IMPLEMENTATIONS ===
// Реализации (inherent impl, затем trait impl)

// === FREE FUNCTIONS ===
// Свободные функции

// === TESTS ===
// Модуль тестов
```

### 2.2 Детальные правила

#### 2.2.1 MODULES

```rust
// === MODULES ===
pub mod submodule1;
pub mod submodule2;
mod private_module;

pub use submodule1::PublicType;
pub use submodule2::*;
```

**Правила:**

- Сначала `pub mod`, затем `mod`
- Алфавитный порядок внутри группы
- Re-exports после объявлений модулей
- Пустая строка между группами

#### 2.2.2 IMPORTS

```rust
// === IMPORTS ===
// Стандартная библиотека
use std::collections::HashMap;
use std::sync::Arc;

// Внешние зависимости
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

// Внутренние модули
use crate::config::Config;
use crate::utils::helper_function;

// Локальные импорты
use super::parent_module::ParentType;
```

**Правила:**

- Группировка: std → внешние → crate → super/self
- Алфавитный порядок внутри группы
- Пустая строка между группами
- Максимум 3 уровня вложенности в use

#### 2.2.3 CONSTANTS

```rust
// === CONSTANTS ===
pub const DEFAULT_TIMEOUT: u64 = 30;
pub const MAX_RETRIES: usize = 3;

const INTERNAL_BUFFER_SIZE: usize = 1024;

pub static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
```

**Правила:**

- Сначала `pub const`, затем `const`, затем `static`
- SCREAMING_SNAKE_CASE для имен
- Группировка по области видимости

#### 2.2.4 TYPE ALIASES

```rust
// === TYPE ALIASES ===
pub type Result<T> = std::result::Result<T, MyError>;
pub type HashMap<K, V> = std::collections::HashMap<K, V>;

type InternalResult = Result<(), InternalError>;
```

#### 2.2.5 ENUMS

```rust
// === ENUMS ===
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Pending,
    Running,
    Completed,
    Failed(String),
}

#[derive(Debug)]
enum InternalState {
    Idle,
    Processing,
}
```

**Правила:**

- Сначала публичные, затем приватные
- Derive атрибуты на отдельной строке
- PascalCase для имен enum и вариантов

#### 2.2.6 STRUCTS

```rust
// === STRUCTS ===
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    timeout: Duration,
}

#[derive(Debug)]
pub struct Builder {
    config: Option<Config>,
}

struct InternalState {
    counter: usize,
}
```

**Правила:**

- Публичные структуры перед приватными
- Публичные поля перед приватными
- Документация для публичных элементов

#### 2.2.7 TRAITS

```rust
// === TRAITS ===
/// Описание трейта
pub trait Processor {
    type Output;
    type Error;
    
    fn process(&self, input: &str) -> Result<Self::Output, Self::Error>;
    
    fn validate(&self, input: &str) -> bool {
        !input.is_empty()
    }
}

#[async_trait]
pub trait AsyncProcessor {
    async fn process_async(&self, input: &str) -> Result<String>;
}
```

#### 2.2.8 IMPLEMENTATIONS

```rust
// === IMPLEMENTATIONS ===
// Inherent implementations (собственные методы)
impl Config {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            timeout: Duration::from_secs(30),
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Builder {
    pub fn new() -> Self {
        Self { config: None }
    }
}

// Trait implementations
impl Default for Config {
    fn default() -> Self {
        Self::new("localhost".to_string(), 8080)
    }
}

impl Processor for MyProcessor {
    type Output = String;
    type Error = MyError;
    
    fn process(&self, input: &str) -> Result<Self::Output, Self::Error> {
        // implementation
    }
}
```

**Правила:**

- Сначала inherent impl, затем trait impl
- Группировка по типам
- Публичные методы перед приватными

#### 2.2.9 FREE FUNCTIONS

```rust
// === FREE FUNCTIONS ===
/// Вспомогательная функция для обработки строк
pub fn process_string(input: &str) -> String {
    input.trim().to_lowercase()
}

/// Асинхронная функция для загрузки данных
pub async fn load_data(url: &str) -> Result<String> {
    // implementation
}

fn internal_helper(data: &[u8]) -> Vec<u8> {
    // implementation
}
```

#### 2.2.10 TESTS

```rust
// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_creation() {
        let config = Config::new("localhost".to_string(), 8080);
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 8080);
    }
    
    #[tokio::test]
    async fn test_async_function() {
        let result = load_data("http://example.com").await;
        assert!(result.is_ok());
    }
}
```

---

## 3. Дополнительные правила

### 3.1 Комментарии

- Используйте `///` для документации публичных элементов
- Используйте `//` для обычных комментариев
- Секционные комментарии: `// === SECTION_NAME ===`

### 3.2 Форматирование

- Используйте `rustfmt` с настройками по умолчанию
- Максимальная длина строки: 100 символов
- Отступы: 4 пробела

### 3.3 Именование

- `snake_case` для переменных и функций
- `PascalCase` для типов и трейтов
- `SCREAMING_SNAKE_CASE` для констант
- Префикс `_` для неиспользуемых переменных

---

## 4. Исключения

### 4.1 Малые файлы (< 50 строк)

Можно опускать секционные комментарии, но сохранять порядок.

### 4.2 Файлы с одним основным типом

Можно группировать связанные impl блоки рядом со struct/enum.

### 4.3 Тестовые файлы

Могут иметь упрощенную структуру с фокусом на тесты.

---

## 5. Примеры

### 5.1 Полный пример файла

```rust
// === MODULES ===
pub mod config;
pub mod processor;

// === IMPORTS ===
use std::time::Duration;
use anyhow::Result;
use crate::config::Config;

// === CONSTANTS ===
pub const DEFAULT_TIMEOUT: u64 = 30;

// === ENUMS ===
#[derive(Debug)]
pub enum Status {
    Active,
    Inactive,
}

// === STRUCTS ===
pub struct Service {
    config: Config,
    status: Status,
}

// === TRAITS ===
pub trait Runnable {
    fn run(&self) -> Result<()>;
}

// === IMPLEMENTATIONS ===
impl Service {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            status: Status::Inactive,
        }
    }
}

impl Runnable for Service {
    fn run(&self) -> Result<()> {
        // implementation
        Ok(())
    }
}

// === FREE FUNCTIONS ===
pub fn create_default_service() -> Service {
    Service::new(Config::default())
}
```

---

## 6. Инструменты

### 6.1 Автоматизация

- **rustfmt**: Автоформатирование
- **clippy**: Линтинг
- **cargo-sort**: Сортировка зависимостей

### 6.2 IDE настройки

- Настройка автоимпортов по группам
- Шаблоны файлов с секциями
- Сниппеты для быстрого создания структуры

---

## 7. Контроль качества

### 7.1 Code Review

- Проверка соблюдения структуры
- Корректность группировки импортов
- Наличие документации для публичных элементов

### 7.2 CI/CD

```yaml
- name: Check formatting
  run: cargo fmt -- --check
  
- name: Run clippy
  run: cargo clippy -- -D warnings
```

---

*Документ подготовлен для обеспечения единообразия и качества кода в Rust проектах.*
