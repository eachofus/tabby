//! Интеграционные тесты для API автодополнения кода
//!
//! Выполняет golden тесты компонента автодополнения Tabby с реальным сервером.
//! Тестирует качество генерации кода через completions API и сравнивает
//! результаты с эталонными снимками для обеспечения стабильности модели.

// === IMPORTS ===
use std::path::PathBuf;

use lazy_static::lazy_static;
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

/// URL для completions API
const COMPLETIONS_API_URL: &str = "http://127.0.0.1:9090/v1/completions";

/// Модель для автодополнения кода (компактная модель для быстрых тестов)
const CODE_MODEL: &str = "TabbyML/StarCoder-1B";

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
    let cargo_path = std::path::Path::new(std::str::from_utf8(&output).expect("Valid path").trim());
    cargo_path
        .parent()
        .expect("Path must have a parent folder")
        .to_path_buf()
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

/// Инициализирует тестовый сервер Tabby для автодополнения
///
/// Запускает сервер в фоновом режиме с моделью для генерации кода.
/// Сервер автоматически завершается при завершении теста благодаря kill_on_drop.
///
/// # Arguments
///
/// * `gpu_device` - Опциональное устройство для вычислений ("cpu", "metal", "cuda")
///
/// # Configuration
///
/// - Модель: TabbyML/StarCoder-1B для автодополнения
/// - Порт: 9090 (константа TEST_SERVER_PORT)
/// - Веб-интерфейс отключен для тестирования
/// - Отключен RAG для предсказуемости результатов
fn initialize_server(gpu_device: Option<&str>) {
    let mut cmd = Command::new(tabby_path());
    cmd.arg("serve")
        .arg("--model")
        .arg(CODE_MODEL)
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
            .expect("Failed to start server");
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
/// Отправляет запрос к completions API и возвращает JSON ответ для сравнения
/// с эталонным снимком. Автоматически отключает RAG для предсказуемости результатов.
///
/// # Arguments
///
/// * `body` - JSON тело запроса к completions API
///
/// # Returns
///
/// JSON ответ от API с результатами автодополнения
///
/// # Process
///
/// 1. Клонирует тело запроса и добавляет debug_options
/// 2. Отправляет первый запрос для отладочной информации
/// 3. Отправляет второй запрос для получения результата
/// 4. Возвращает JSON ответ для сравнения со снимком
///
/// # Debug Options
///
/// Автоматически отключает retrieval augmented code completion для
/// обеспечения детерминированности результатов тестирования.
async fn golden_test(body: serde_json::Value) -> serde_json::Value {
    let mut body = body.clone();
    
    // Отключаем RAG для предсказуемости результатов
    body.as_object_mut().unwrap().insert(
        "debug_options".to_owned(),
        json!({
            "disable_retrieval_augmented_code_completion": true
        }),
    );

    // Первый запрос для получения отладочной информации
    let resp = CLIENT
        .post(COMPLETIONS_API_URL)
        .json(&body)
        .send()
        .await
        .unwrap();

    let info = resp.text().await.unwrap();
    eprintln!("Debug info: {info}");

    // Второй запрос для получения фактического результата
    let actual: serde_json::Value = CLIENT
        .post(COMPLETIONS_API_URL)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        
    actual
}

// === MACROS ===
// Макрос для создания golden тестов автодополнения
//
// Выполняет golden тест с заданным JSON запросом и сравнивает результат
// с сохраненным снимком используя insta crate. Автоматически маскирует
// поле id для стабильности тестов между запусками.
macro_rules! assert_golden {
    ($expr:expr) => {
        insta::assert_yaml_snapshot!(golden_test($expr).await, {
            ".id" => "test-id"
        });
    };
}

// === TESTS ===
/// Тесты автодополнения для macOS ARM64 с Metal ускорением
///
/// Выполняет набор golden тестов на macOS с использованием Metal для GPU ускорения.
/// Тестирует различные сценарии автодополнения кода и проверяет качество генерации.
///
/// # Test Cases
///
/// 1. Функция Fibonacci - тестирует понимание рекурсивных алгоритмов
/// 2. Парсер расходов - тестирует работу с строками, датами и форматированием
///
/// # Requirements
///
/// - macOS с архитектурой ARM64 (Apple Silicon)
/// - Доступность Metal для GPU вычислений
/// - Модель TabbyML/StarCoder-1B должна быть доступна
///
/// # Code Scenarios
///
/// Тесты покрывают типичные задачи программирования:
/// - Математические алгоритмы (Fibonacci)
/// - Обработка текстовых данных (парсинг CSV-подобных форматов)
/// - Работа с датами и валютами
/// - Обработка комментариев и специальных символов
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test]
#[serial]
async fn run_golden_tests() {
    wait_for_server(Some("metal")).await;

    // Тест: завершение функции Fibonacci
    // Проверяет понимание рекурсивных алгоритмов и базовых случаев
    assert_golden!(json!({
        "language": "python",
        "seed": 0,
        "segments": {
            "prefix": "def fib(n):\n    ",
            "suffix": "\n        return fib(n - 1) + fib(n - 2)"
        }
    }));

    // Тест: парсер строки расходов
    // Проверяет способность генерировать код для обработки структурированных данных
    // включая работу с датами, числами, валютами и фильтрацию комментариев
    assert_golden!(json!({
        "language": "python",
        "seed": 0,
        "segments": {
            "prefix": "import datetime\n\ndef parse_expenses(expenses_string):\n    \"\"\"Parse the list of expenses and return the list of triples (date, value, currency).\n    Ignore lines starting with #.\n    Parse the date using datetime.\n    Example expenses_string:\n        2016-01-02 -34.01 USD\n        2016-01-03 2.59 DKK\n        2016-01-03 -2.72 EUR\n    \"\"\"\n    for line in expenses_string.split('\\n'):\n        "
        }
    }));
}

/// Тесты автодополнения с CPU вычислениями
///
/// Выполняет golden тесты с использованием CPU для вычислений.
/// Подходит для сред без GPU или для тестирования совместимости CPU режима.
///
/// # Test Cases
///
/// 1. Функция проверки простых чисел - базовые математические алгоритмы
/// 2. Подсчет частоты символов - работа со строками и словарями
///
/// # Requirements
///
/// - Модель TabbyML/StarCoder-1B должна быть доступна
/// - Достаточно CPU ресурсов для инференса модели
///
/// # Performance Notes
///
/// CPU тесты выполняются медленнее GPU версий, но обеспечивают
/// совместимость на системах без специализированного оборудования.
#[tokio::test]
#[serial]
async fn run_golden_tests_cpu() {
    wait_for_server(Some("cpu")).await;

    // Тест: функция проверки простых чисел
    // Проверяет генерацию базовых математических алгоритмов
    assert_golden!(json!({
        "language": "python",
        "seed": 0,
        "segments": {
            "prefix": "def is_prime(n):\n",
        }
    }));

    // Тест: подсчет частоты символов в строке
    // Проверяет работу со словарями и итерацией по строкам
    assert_golden!(json!({
        "language": "python",
        "seed": 0,
        "segments": {
            "prefix": "def char_frequencies(str):\n  freqs = {}\n  ",
        }
    }));
}
