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
    /// Formats an `EmbedError` into a human-readable message prefixed with `embed:`.
    ///
    /// Each variant is rendered with a clear description and any associated data:
    /// - `InvalidInput(msg)` -> `embed: invalid input: {msg}`
    /// - `ProviderUnavailable(msg)` -> `embed: provider unavailable: {msg}`
    /// - `DimensionMismatch { expected, actual }` -> `embed: dimension mismatch: expected {expected}, got {actual}`
    /// - `InternalError(msg)` -> `embed: internal error: {msg}`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fmt::Write;
    ///
    /// let err = crate::EmbedError::InvalidInput("empty".into());
    /// let s = format!("{}", err);
    /// assert_eq!(s, "embed: invalid input: empty");
    /// ```
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

    /// Generates embedding vectors for a batch of text inputs.
    ///
    /// The default implementation calls `embed` for each input sequentially; providers may override
    /// to perform optimized batch processing.
    ///
    /// # Examples
    ///
    /// ```
    /// struct Dummy;
    /// impl EmbeddingProvider for Dummy {
    ///     fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
    ///         Ok(vec![0.0; 4])
    ///     }
    ///     fn dimensions(&self) -> u32 { 4 }
    /// }
    ///
    /// let provider = Dummy;
    /// let batch = ["hello", "world"];
    /// let embeddings = provider.embed_batch(&batch).unwrap();
    /// assert_eq!(embeddings.len(), 2);
    /// assert_eq!(embeddings[0].len(), 4);
    /// ```
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|t| self.embed(t)).collect()
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

    impl EmbeddingProvider for DummyEmbeddingProvider {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
            Ok(vec![0.0; 4])
        }

        /// Returns the dimensionality of embeddings produced by this provider.
        ///
        /// # Examples
        ///
        /// ```
        /// let provider = DummyEmbeddingProvider;
        /// assert_eq!(provider.dimensions(), 4);
        /// ```
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
        /// Asserts at compile time that a type implements `Send` and `Sync`.

///

/// This function has no runtime behavior; invoking it enforces the `Send + Sync` trait bounds for `T`.

///

/// # Examples

///

/// ```

/// struct Dummy;

///

/// // Compile will fail if `Dummy` does not implement `Send + Sync`.

/// assert_send_sync::<Dummy>();

/// ```
fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DummyEmbeddingProvider>();

        /// Compile-time assertion that a trait object is usable as `dyn EmbeddingProvider + Send + Sync`.

///

/// This function is a no-op at runtime and exists only to require the caller provide

/// a reference whose dynamic type implements both `Send` and `Sync`.

///

/// # Examples

///

/// ```

/// // Ensure `provider` can be used as a `dyn EmbeddingProvider + Send + Sync`.

/// // let provider: &dyn EmbeddingProvider = /* ... */;

/// // assert_dyn_send_sync(provider);

/// ```
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