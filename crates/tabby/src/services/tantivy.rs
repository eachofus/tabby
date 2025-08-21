//! Провайдер асинхронного доступа к поисковому индексу Tantivy
//!
//! Обеспечивает потокобезопасный доступ к индексу с автоматической фоновой загрузкой
//! и проверкой совместимости схемы. Поддерживает множественные читающие потоки
//! и автоматические повторы при недоступности индекса.

// === IMPORTS ===
use std::{sync::Arc, time::Duration};

use tabby_common::{index::IndexSchema, path};
use tantivy::{Index, IndexReader};
use tokio::sync::RwLock;
use tracing::debug;

// === STRUCTS ===
/// Использует RwLock для эффективного чтения индекса множественными потоками
/// и фоновую задачу для асинхронной инициализации.
///
/// # Поля
///
/// * `provider` - Потокобезопасный доступ к IndexReader
/// * `loader` - Фоновая задача для загрузки индекса
///
/// # Examples
///
/// ```rust
/// let provider = IndexReaderProvider::default();
/// let reader_guard = provider.reader().await;
/// if let Some(reader) = reader_guard.as_ref() {
///     // Использование индекса для поиска
/// }
/// ```
pub struct IndexReaderProvider {
    /// Потокобезопасный контейнер для IndexReader
    provider: Arc<RwLock<Option<IndexReader>>>,
    /// Фоновая задача загрузки индекса
    loader: tokio::task::JoinHandle<()>,
}

// === IMPLEMENTATIONS ===
impl IndexReaderProvider {
    /// Получает guard для чтения IndexReader
    ///
    /// Возвращает RwLockReadGuard, который позволяет безопасно читать
    /// индекс из множественных потоков. Если индекс еще не загружен,
    /// возвращает None внутри Option.
    ///
    /// # Returns
    ///
    /// Future, который разрешается в RwLockReadGuard с Option<IndexReader>
    pub fn reader<'a>(
        &'a self,
    ) -> impl futures::Future<Output = tokio::sync::RwLockReadGuard<'a, Option<IndexReader>>> {
        self.provider.read()
    }

    /// Синхронная загрузка IndexReader с проверкой схемы
    ///
    /// Открывает индекс из директории и проверяет совместимость схемы.
    /// Используется внутренне для инициализации индекса.
    ///
    /// # Returns
    ///
    /// IndexReader при успешной загрузке
    ///
    /// # Errors
    ///
    /// Возвращает ошибку если:
    /// - Директория индекса недоступна
    /// - Схема индекса не совпадает с ожидаемой
    /// - Ошибка создания IndexReader
    fn load() -> anyhow::Result<IndexReader> {
        // Открываем индекс из стандартной директории
        let index = Index::open_in_dir(path::index_dir())?;

        // Проверяем совместимость схемы индекса для предотвращения ошибок поиска
        if index.schema() != IndexSchema::instance().schema {
            return Err(anyhow::anyhow!("Index schema mismatch"));
        }

        // Создаем reader с настройками по умолчанию
        Ok(index.reader_builder().try_into()?)
    }

    /// Асинхронная загрузка с повторами при ошибках
    ///
    /// Бесконечно пытается загрузить индекс с интервалом в 60 секунд
    /// между попытками. Используется для фоновой инициализации.
    ///
    /// # Returns
    ///
    /// IndexReader после успешной загрузки (никогда не возвращает ошибку)
    async fn load_async() -> IndexReader {
        loop {
            if let Ok(provider) = Self::load() {
                debug!("Index is ready, enabling search...");
                return provider;
            }

            // Ждем 60 секунд перед следующей попыткой загрузки
            // Это предотвращает чрезмерную нагрузку на систему при недоступности индекса
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}

impl Default for IndexReaderProvider {
    /// Создает новый провайдер с фоновой загрузкой индекса
    ///
    /// Инициализирует пустой провайдер и запускает фоновую задачу
    /// для асинхронной загрузки индекса. Индекс становится доступным
    /// после успешной загрузки в фоновом режиме.
    ///
    /// # Returns
    ///
    /// Новый экземпляр IndexReaderProvider с запущенной фоновой загрузкой
    fn default() -> Self {
        // Создаем пустой контейнер для IndexReader
        let provider = Arc::new(RwLock::new(None));
        let cloned_provider = provider.clone();
        
        // Запускаем фоновую задачу для загрузки индекса
        let loader = tokio::spawn(async move {
            let doc = Self::load_async().await;
            // Атомарно обновляем провайдер с загруженным индексом
            *cloned_provider.write().await = Some(doc);
        });

        Self { provider, loader }
    }
}

impl Drop for IndexReaderProvider {
    /// Корректно завершает фоновую задачу при уничтожении провайдера
    ///
    /// Отменяет фоновую задачу загрузки для предотвращения утечек ресурсов
    /// и зависших задач после уничтожения провайдера.
    fn drop(&mut self) {
        // Принудительно завершаем фоновую задачу
        self.loader.abort()
    }
}
