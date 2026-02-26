//! Semantic index built on Zig embedding kernels.
//!
//! This module is feature-gated behind `embeddings`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::DocumentIndex;
use markymark_core::prelude::*;
use markymark_kernels::embed::EmbeddingIndex as ZigEmbeddingIndex;

mod helpers;
mod ops_add_remove;
mod ops_update_search;
mod types;
pub(super) use helpers::{
    compute_fetch_k, fallback_heading, fnv1a32, jaccard_similarity, token_hashes,
};
pub use types::{DuplicateMatch, SearchResult, SemanticEntry};

/// Semantic index backed by [`ZigEmbeddingIndex`].
///
/// Stores entry metadata keyed by stable IDs and filters stale embedding IDs at
/// query time. This supports document replacement/removal even though the
/// current Zig embedding index API does not expose a delete operation.
pub struct SemanticIndex {
    provider: Arc<dyn EmbeddingProvider>,
    index: ZigEmbeddingIndex,
    entries_by_id: HashMap<String, SemanticEntry>,
    doc_to_ids: HashMap<DocumentUri, Vec<String>>,
    doc_token_sets: HashMap<DocumentUri, BTreeSet<u32>>,
}

impl SemanticIndex {
    /// Create a semantic index using the provided embedding backend.
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Result<Self, EmbedError> {
        let dims = provider.dimensions();
        if dims == 0 {
            return Err(EmbedError::InvalidInput(
                "embedding dimensions must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            provider,
            index: ZigEmbeddingIndex::new(dims)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?,
            entries_by_id: HashMap::new(),
            doc_to_ids: HashMap::new(),
            doc_token_sets: HashMap::new(),
        })
    }

    /// Number of active semantic entries.
    pub fn entry_count(&self) -> usize {
        self.entries_by_id.len()
    }
}


#[cfg(test)]
mod tests;
