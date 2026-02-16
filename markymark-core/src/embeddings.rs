//! Embedding provider trait for vector-based semantic operations.
//!
//! [`EmbeddingProvider`] provides a source-agnostic interface for generating
//! text embeddings. The provider decision (local ONNX, API, TF-IDF) is
//! deferred to implementation time.

use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by embedding operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedError {
    /// Invalid or malformed input.
    InvalidInput(String),
    /// The embedding model or provider is unavailable.
    ProviderUnavailable(String),
    /// Dimension mismatch between query and index.
    DimensionMismatch {
        /// Expected dimensionality.
        expected: u32,
        /// Actual dimensionality provided.
        actual: u32,
    },
    /// Internal embedding failure.
    InternalError(String),
}

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "embed: invalid input: {msg}"),
            Self::ProviderUnavailable(msg) => write!(f, "embed: provider unavailable: {msg}"),
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "embed: dimension mismatch: expected {expected}, got {actual}"
                )
            }
            Self::InternalError(msg) => write!(f, "embed: internal error: {msg}"),
        }
    }
}

impl std::error::Error for EmbedError {}

// ---------------------------------------------------------------------------
// EmbeddingProvider trait
// ---------------------------------------------------------------------------

/// Source-agnostic interface for generating text embeddings.
///
/// Implementations must be `Send + Sync` and object-safe.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for a single text input.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    /// Generate embedding vectors for a batch of text inputs.
    ///
    /// Default implementation calls [`embed`](Self::embed) sequentially.
    /// Implementations may override for batch-optimized providers.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Return the dimensionality of embedding vectors produced by this provider.
    fn dimensions(&self) -> u32;
}

// ---------------------------------------------------------------------------
// Zig SIMD embedding index (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "zig-kernels")]
mod zig_embedding_index {
    use std::sync::Mutex;

    use markymark_kernels::embed;

    use super::EmbedError;

    /// Search result from a [`ZigEmbeddingIndex`] query.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ZigSearchResult {
        /// The ID of the matching entry.
        pub id: String,
        /// Cosine similarity score.
        pub score: f32,
    }

    /// Thread-safe wrapper around the Zig SIMD embedding index.
    ///
    /// The inner `EmbeddingIndex` is `!Send + !Sync` (raw Zig pointer), so we
    /// wrap it in a `Mutex` to allow safe sharing across threads.
    pub struct ZigEmbeddingIndex {
        inner: Mutex<embed::EmbeddingIndex>,
        dims: u32,
    }

    // SAFETY: The Mutex ensures exclusive access to the inner EmbeddingIndex.
    // The Zig handle is only accessed through the lock guard.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe impl Send for ZigEmbeddingIndex {}
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe impl Sync for ZigEmbeddingIndex {}

    impl ZigEmbeddingIndex {
        /// Create a new embedding index for vectors of the given dimensionality.
        pub fn new(dims: u32) -> Result<Self, EmbedError> {
            let inner = embed::EmbeddingIndex::new(dims)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
            Ok(Self {
                inner: Mutex::new(inner),
                dims,
            })
        }

        /// Add an embedding with the given ID.
        pub fn add(&mut self, id: &str, embedding: &[f32]) -> Result<(), EmbedError> {
            let mut guard = self.inner.lock().unwrap();
            guard
                .add(id, embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))
        }

        /// Search for the top-K most similar embeddings.
        pub fn search(&self, query: &[f32], k: u32) -> Result<Vec<ZigSearchResult>, EmbedError> {
            let guard = self.inner.lock().unwrap();
            let results = guard
                .search(query, k)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
            Ok(results
                .into_iter()
                .map(|r| ZigSearchResult {
                    id: r.id,
                    score: r.score,
                })
                .collect())
        }

        /// Return the number of entries in the index.
        pub fn count(&self) -> u32 {
            let guard = self.inner.lock().unwrap();
            guard.count()
        }

        /// Return the dimensionality of vectors in this index.
        pub fn dimensions(&self) -> u32 {
            self.dims
        }
    }
}

#[cfg(feature = "zig-kernels")]
pub use zig_embedding_index::{ZigEmbeddingIndex, ZigSearchResult};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyEmbeddingProvider;

    impl EmbeddingProvider for DummyEmbeddingProvider {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
            Ok(vec![0.0; 4])
        }

        fn dimensions(&self) -> u32 {
            4
        }
    }

    #[test]
    fn test_embedding_provider_trait_object() {
        // Verifies EmbeddingProvider is object-safe (dyn-compatible).
        let provider: Box<dyn EmbeddingProvider> = Box::new(DummyEmbeddingProvider);
        let result = provider.embed("hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 4);
    }

    #[test]
    fn test_embedding_provider_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DummyEmbeddingProvider>();

        fn assert_dyn_send_sync(_: &(dyn EmbeddingProvider + Send + Sync)) {}
        let provider = DummyEmbeddingProvider;
        assert_dyn_send_sync(&provider);
    }

    #[test]
    fn test_embedding_provider_dimensions() {
        let provider = DummyEmbeddingProvider;
        assert_eq!(provider.dimensions(), 4);
    }

    #[test]
    fn test_embedding_provider_batch_default() {
        let provider = DummyEmbeddingProvider;
        let results = provider.embed_batch(&["hello", "world"]).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 4);
        assert_eq!(results[1].len(), 4);
    }

    #[test]
    fn test_embed_error_display() {
        let err = EmbedError::InvalidInput("empty".to_string());
        assert_eq!(err.to_string(), "embed: invalid input: empty");

        let err = EmbedError::ProviderUnavailable("offline".to_string());
        assert_eq!(err.to_string(), "embed: provider unavailable: offline");

        let err = EmbedError::DimensionMismatch {
            expected: 384,
            actual: 768,
        };
        assert_eq!(
            err.to_string(),
            "embed: dimension mismatch: expected 384, got 768"
        );

        let err = EmbedError::InternalError("oom".to_string());
        assert_eq!(err.to_string(), "embed: internal error: oom");
    }
}

// ---------------------------------------------------------------------------
// Zig-kernels embedding tests
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "zig-kernels"))]
mod zig_tests {
    use super::*;

    #[test]
    fn test_zig_embedding_index_lifecycle() {
        let mut idx = ZigEmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(idx.count(), 2);
        assert_eq!(idx.dimensions(), 4);
    }

    #[test]
    fn test_zig_embedding_index_search() {
        let mut idx = ZigEmbeddingIndex::new(4).unwrap();
        idx.add("doc1", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx.add("doc2", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results[0].id, "doc1");
        assert!(results[0].score > 0.99);
    }

    #[test]
    fn test_zig_embedding_index_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ZigEmbeddingIndex>();
    }

    #[test]
    fn test_zig_embedding_index_zero_dims() {
        assert!(ZigEmbeddingIndex::new(0).is_err());
    }

    #[test]
    fn test_zig_embedding_index_empty_search() {
        let idx = ZigEmbeddingIndex::new(4).unwrap();
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }
}
