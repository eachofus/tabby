//! Интеграционные тесты для чат API
//!
//! Выполняет golden тесты чат-компонента Tabby с реальным сервером.
//! Тестирует потоковые ответы через Server-Sent Events и сравнивает
//! результаты с эталонными снимками для обеспечения стабильности API.

// === IMPORTS ===
use std::path::PathBuf;

use futures::StreamExt;
use lazy_static::lazy_static;
use reqwest_eventsource::{Event, EventSource};
use serde::Deserialize;
use serde_json::json;
use serial_test::serial;
use tokio::{
    process::Command,
    time::{sleep, Duration},
};

// === CONSTANTS ===
// Глобальный HTTP клиент для всех тестов
//
// Переиспользуется между тестами для эффективности и избежания
// создания множественных соединений к тестовому серверу.
lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
}

/// Порт для тестового сервера Tabby
const TEST_SERVER_PORT: u16 = 9090;

/// Интервал проверки готовности сервера
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// URL для проверки здоровья сервера
const HEALTH_CHECK_URL: &str = "http://127.0.0.1:9090/v1/health";

/// URL для чат API
const CHAT_API_URL: &str = "http://127.0.0.1:9090/v1/chat/completions";

// === STRUCTS ===
/// Фрагмент ответа чат API в потоковом режиме
///
/// Представляет один элемент в потоке Server-Sent Events от чат API.
/// Содержит частичное обновление генерируемого ответа.
#[derive(Deserialize)]
pub struct ChatCompletionChunk {
    /// Массив вариантов ответа (всегда содержит один элемент)
    choices: [ChatCompletionChoice; 1],
}

/// Вариант ответа в чат completion
///
/// Содержит дельта-обновление для генерируемого сообщения.
/// В потоковом режиме каждый фрагмент содержит только новую часть текста.
#[derive(Deserialize)]
pub struct ChatCompletionChoice {
    /// Дельта-изменение в сообщении
    delta: ChatCompletionDelta,
}

/// Дельта-изменение в сообщении чата
///
/// Представляет инкрементальное обновление содержимого сообщения.
/// В потоковом режиме содержит только новую часть текста или None для служебных сообщений.
#[derive(Deserialize)]
pub struct ChatCompletionDelta {
    /// Новая часть содержимого сообщения (может отсутствовать)
    content: Option<String>,
}

// === FREE FUNCTIONS ===
/// Определяет корневую директорию workspace
///
/// Использует cargo для определения местоположения корневого Cargo.toml
/// и возвращает путь к директории workspace.
///
/// # Returns
///
/// Путь к корневой директории проекта
///
/// # Panics
///
/// Паникует если не удалось выполнить команду cargo или разобрать вывод
fn workspace_dir() -> PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = std::path::Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

/// Возвращает путь к исполняемому файлу Tabby
///
/// Определяет местоположение скомпилированного бинарника Tabby
/// в директории target/debug проекта.
///
/// # Returns
///
/// Путь к исполняемому файлу tabby
fn tabby_path() -> PathBuf {
    workspace_dir().join("target/debug/tabby")
}

/// Инициализирует тестовый сервер Tabby
///
/// Запускает сервер в фоновом режиме с указанными параметрами.
/// Сервер автоматически завершается при завершении теста благодаря kill_on_drop.
///
/// # Arguments
///
/// * `gpu_device` - Опциональное устройство для вычислений ("cpu", "metal", "cuda")
///
/// # Configuration
///
/// - Модель: TabbyML/Mistral-7B для чата
/// - Порт: 9090 (константа TEST_SERVER_PORT)
/// - Веб-интерфейс отключен для тестирования
fn initialize_server(gpu_device: Option<&str>) {
    let mut cmd = Command::new(tabby_path());
    cmd.arg("serve")
        .arg("--chat-model")
        .arg("TabbyML/Mistral-7B")
        .arg("--no-webserver")
        .arg("--port")
        .arg(TEST_SERVER_PORT.to_string())
        .kill_on_drop(true);

    if let Some(gpu_device) = gpu_device {
        cmd.arg("--device").arg(gpu_device);
    }

    tokio::task::spawn(async move {
        cmd.spawn()
            .expect("Failed to start server")
            .wait()
            .await
            .unwrap();
    });
}

/// Ожидает готовности тестового сервера
///
/// Запускает сервер и периодически проверяет его готовность через health check endpoint.
/// Блокирует выполнение до тех пор, пока сервер не станет доступен.
///
/// # Arguments
///
/// * `gpu_device` - Устройство для вычислений (передается в initialize_server)
///
/// # Behavior
///
/// - Проверяет готовность каждые 5 секунд
/// - Выводит статус ожидания в консоль
/// - Завершается при получении успешного ответа от /v1/health
async fn wait_for_server(gpu_device: Option<&str>) {
    initialize_server(gpu_device);

    loop {
        println!("Waiting for server to start...");
        match reqwest::get(HEALTH_CHECK_URL).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    break;
                }
            }
            Err(e) => {
                println!("Waiting for server to start: {e:?}");
            }
        }
        sleep(HEALTH_CHECK_INTERVAL).await;
    }
}

/// Выполняет golden тест с заданным телом запроса
///
/// Отправляет запрос к чат API, получает потоковый ответ через Server-Sent Events
/// и собирает полный текст ответа для сравнения с эталонным снимком.
///
/// # Arguments
///
/// * `body` - JSON тело запроса к чат API
///
/// # Returns
///
/// Полный текст ответа, собранный из всех фрагментов потока
///
/// # Process
///
/// 1. Создает EventSource для потокового чтения
/// 2. Обрабатывает каждое событие в потоке
/// 3. Извлекает содержимое из ChatCompletionChunk
/// 4. Собирает полный ответ из фрагментов
/// 5. Обрабатывает ошибки и завершение потока
async fn golden_test(body: serde_json::Value) -> String {
    let mut es = EventSource::new(
        CLIENT
            .post(CHAT_API_URL)
            .json(&body),
    )
    .unwrap();

    let mut actual = String::new();
    
    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Open) => {
                // Соединение установлено, продолжаем
            }
            Ok(Event::Message(message)) => {
                let chunk: ChatCompletionChunk = serde_json::from_str(&message.data).unwrap();
                if let Some(content) = &chunk.choices[0].delta.content {
                    actual += content;
                }
            }
            Err(e) => {
                match e {
                    reqwest_eventsource::Error::StreamEnded => {
                        // Нормальное завершение потока
                        break;
                    }
                    reqwest_eventsource::Error::InvalidStatusCode(code, resp) => {
                        let resp = resp.text().await.unwrap();
                        println!("Error: {code} {resp:?}");
                    }
                    e => {
                        println!("Error: {e:?}");
                    }
                }
                break;
            }
        }
    }

    actual
}

// === MACROS ===
// Макрос для создания golden тестов
//
// Выполняет golden тест с заданным JSON запросом и сравнивает результат
// с сохраненным снимком используя insta crate. Автоматически создает или
// обновляет снимки при изменении вывода.
macro_rules! assert_golden {
    ($expr:expr) => {
        insta::assert_yaml_snapshot!(golden_test($expr).await);
    };
}

// === TESTS ===
/// Тесты чата для macOS ARM64 с Metal ускорением
///
/// Выполняет набор golden тестов на macOS с использованием Metal для GPU ускорения.
/// Тестирует различные типы запросов к чат API и проверяет стабильность ответов.
///
/// # Test Cases
///
/// 1. Преобразование списка строк в числа в Python
/// 2. Парсинг email адресов с помощью regex
///
/// # Requirements
///
/// - macOS с архитектурой ARM64 (Apple Silicon)
/// - Доступность Metal для GPU вычислений
/// - Модель TabbyML/Mistral-7B должна быть доступна
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test]
#[serial]
async fn run_chat_golden_tests() {
    wait_for_server(Some("metal")).await;

    // Тест: преобразование строк в числа в Python
    assert_golden!(json!({
        "seed": 0,
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "How to convert a list of string to numbers in python"
            }
        ]
    }));

    // Тест: парсинг email с regex
    assert_golden!(json!({
        "seed": 0,
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "How to parse email address with regex"
            }
        ]
    }));
}

/// Тесты чата с CPU вычислениями
///
/// Выполняет golden тесты с использованием CPU для вычислений.
/// Подходит для сред без GPU или для тестирования совместимости CPU режима.
///
/// # Test Cases
///
/// 1. Простой диалоговый запрос "How are you?"
///
/// # Requirements
///
/// - Модель TabbyML/Mistral-7B должна быть доступна
/// - Достаточно CPU ресурсов для инференса модели
#[tokio::test]
#[serial]
async fn run_chat_golden_tests_cpu() {
    wait_for_server(Some("cpu")).await;

    // Тест: простой диалог
    assert_golden!(json!({
        "seed": 0,
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "How are you?"
            }
        ]
    }));
}
