//! Responsible for scheduling all of the background jobs for tabby.
//! Includes syncing respositories and updating indices.

// === MODULES ===
mod code;
mod indexer;
mod structured_doc;
mod tantivy_utils;

#[cfg(test)]
mod indexer_tests;

#[cfg(test)]
mod testutils;

pub mod public {
    // === IMPORTS ===
    use super::*;
    use indexer::IndexGarbageCollector;

    // === RE-EXPORTS ===
    pub use super::{
        code::CodeIndexer,
        structured_doc::public::{
            StructuredDoc, StructuredDocCommitFields, StructuredDocFields,
            StructuredDocGarbageCollector, StructuredDocIndexer, StructuredDocIngestedFields,
            StructuredDocIssueFields, StructuredDocPageFields, StructuredDocPullDocumentFields,
            StructuredDocState, StructuredDocWebFields, KIND_COMMIT as STRUCTURED_DOC_KIND_COMMIT,
        },
    };

    // === FREE FUNCTIONS ===
    pub fn run_index_garbage_collection(active_sources: Vec<String>) -> anyhow::Result<()> {
        let index_garbage_collector = IndexGarbageCollector::new();
        index_garbage_collector.garbage_collect(&active_sources)?;
        index_garbage_collector.commit();
        Ok(())
    }
}

// === IMPORTS ===
use indexer::{IndexAttributeBuilder, Indexer};
