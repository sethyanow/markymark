//! Embedding provider trait for vector-based semantic operations.
//!
//! [`EmbeddingProvider`] provides a source-agnostic interface for generating
//! text embeddings. The provider decision (local ONNX, API, TF-IDF) is
//! deferred to implementation time.

use async_trait::async_trait;
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
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for a single text input.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    /// Generate embedding vectors for a batch of text inputs.
    ///
    /// Default implementation calls [`embed`](Self::embed) sequentially.
    /// Implementations may override for batch-optimized providers.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let mut results = Vec::with_capacity(texts.len());
        for t in texts {
            results.push(self.embed(t).await?);
        }
        Ok(results)
    }

    /// Return the dimensionality of embedding vectors produced by this provider.
    fn dimensions(&self) -> u32;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyEmbeddingProvider;

    #[async_trait]
    impl EmbeddingProvider for DummyEmbeddingProvider {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
            Ok(vec![0.0; 4])
        }

        fn dimensions(&self) -> u32 {
            4
        }
    }

    #[tokio::test]
    async fn test_embedding_provider_trait_object() {
        // Verifies EmbeddingProvider is object-safe (dyn-compatible).
        let provider: Box<dyn EmbeddingProvider> = Box::new(DummyEmbeddingProvider);
        let result = provider.embed("hello").await;
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

    #[tokio::test]
    async fn test_embedding_provider_batch_default() {
        let provider = DummyEmbeddingProvider;
        let results = provider.embed_batch(&["hello", "world"]).await.unwrap();
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
