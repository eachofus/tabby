// === IMPORTS ===
use std::{sync::Arc, time::Duration};

use anyhow::Result;
use futures::Future;
use tantivy::{Index, IndexReader};
use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::debug;

use tabby_common::{index::IndexSchema, path};

// === STRUCTS ===
/// Provider for IndexReader with automatic loading and reloading capabilities
/// 
/// This struct manages an IndexReader instance that is loaded asynchronously
/// and can be accessed through a read-write lock. It automatically retries
/// loading the index if it fails initially.
pub struct IndexReaderProvider {
    provider: Arc<RwLock<Option<IndexReader>>>,
    loader: tokio::task::JoinHandle<()>,
}

// === IMPLEMENTATIONS ===
impl IndexReaderProvider {
    /// Returns a future that resolves to a read guard for the IndexReader
    /// 
    /// The returned guard allows read-only access to the optional IndexReader.
    /// If the index is not yet loaded, the Option will be None.
    pub fn reader(&self) -> impl Future<Output = RwLockReadGuard<'_, Option<IndexReader>>> {
        self.provider.read()
    }

    /// Attempts to load an IndexReader from the configured index directory
    /// 
    /// # Errors
    /// 
    /// Returns an error if:
    /// - The index directory cannot be opened
    /// - The index schema doesn't match the expected schema
    /// - The IndexReader cannot be created
    fn load() -> Result<IndexReader> {
        let index = Index::open_in_dir(path::index_dir())?;

        if index.schema() != IndexSchema::instance().schema {
            return Err(anyhow::anyhow!("Index schema mismatch"));
        }

        Ok(index.reader_builder().try_into()?)
    }

    /// Asynchronously loads an IndexReader with retry logic
    /// 
    /// This function will continuously attempt to load the index every 60 seconds
    /// until it succeeds. It logs when the index becomes ready.
    async fn load_async() -> IndexReader {
        loop {
            if let Ok(provider) = Self::load() {
                debug!("Index is ready, enabling search...");
                return provider;
            }

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}

impl Default for IndexReaderProvider {
    /// Creates a new IndexReaderProvider with automatic loading
    /// 
    /// The provider starts with no IndexReader loaded and spawns a background
    /// task to load it asynchronously. The loading task will retry indefinitely
    /// until successful.
    fn default() -> Self {
        let provider = Arc::new(RwLock::new(None));
        let cloned_provider = provider.clone();
        let loader = tokio::spawn(async move {
            let doc = Self::load_async().await;
            *cloned_provider.write().await = Some(doc);
        });

        Self { provider, loader }
    }
}

impl Drop for IndexReaderProvider {
    /// Aborts the background loading task when the provider is dropped
    fn drop(&mut self) {
        self.loader.abort()
    }
}
