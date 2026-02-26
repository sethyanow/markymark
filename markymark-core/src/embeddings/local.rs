//! Local ONNX embedding provider using fastembed-rs.
//!
//! Implements [`EmbeddingProvider`] using the all-MiniLM-L6-v2 model via
//! [fastembed](https://docs.rs/fastembed). Model auto-downloads on first use
//! to `~/.cache/markymark/models/`.
//!
//! Requires the `local-embeddings` feature flag.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::{EmbedError, EmbeddingProvider};

/// Dimensionality of all-MiniLM-L6-v2 embeddings.
const DIMS: u32 = 384;

/// Local ONNX embedding provider using all-MiniLM-L6-v2.
///
/// Wraps [`fastembed::TextEmbedding`] with `spawn_blocking` for async
/// compatibility. Thread-safe via `Arc<Mutex<_>>` (fastembed requires `&mut self`).
pub struct LocalOnnxProvider {
    model: Arc<Mutex<TextEmbedding>>,
    cache_dir: PathBuf,
}

impl std::fmt::Debug for LocalOnnxProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalOnnxProvider")
            .field("model", &"all-MiniLM-L6-v2")
            .field("dimensions", &DIMS)
            .field("cache_dir", &self.cache_dir)
            .finish()
    }
}

impl LocalOnnxProvider {
    /// Create a new [`LocalOnnxProvider`].
    ///
    /// Downloads the all-MiniLM-L6-v2 model on first use if not cached.
    /// Pass `None` for `cache_dir` to use the default `~/.cache/markymark/models/`.
    ///
    /// Returns [`EmbedError::ProviderUnavailable`] if the model cannot be loaded
    /// (network failure, permissions, corrupt cache).
    pub fn new(cache_dir: Option<PathBuf>) -> Result<Self, EmbedError> {
        let cache_dir = match cache_dir {
            Some(dir) => dir,
            None => default_cache_dir()?,
        };

        std::fs::create_dir_all(&cache_dir).map_err(|e| {
            EmbedError::ProviderUnavailable(format!(
                "cannot create cache directory {}: {e}",
                cache_dir.display()
            ))
        })?;

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_cache_dir(cache_dir.clone())
                .with_show_download_progress(true),
        )
        .map_err(|e| {
            EmbedError::ProviderUnavailable(format!(
                "failed to load all-MiniLM-L6-v2 model: {e}"
            ))
        })?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            cache_dir,
        })
    }
}

#[async_trait]
impl EmbeddingProvider for LocalOnnxProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        validate_text(text)?;

        let model = Arc::clone(&self.model);
        let text = text.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut guard = model.lock().map_err(|e| {
                EmbedError::InternalError(format!("model mutex poisoned: {e}"))
            })?;
            let mut results = guard.embed(vec![text], None).map_err(|e| {
                EmbedError::InternalError(format!("embedding inference failed: {e}"))
            })?;
            if results.is_empty() {
                return Err(EmbedError::InternalError(
                    "model returned empty results".into(),
                ));
            }
            Ok(results.remove(0))
        })
        .await
        .map_err(|e| EmbedError::InternalError(format!("spawn_blocking join error: {e}")))?
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        for (i, t) in texts.iter().enumerate() {
            validate_text_at(t, i)?;
        }

        let model = Arc::clone(&self.model);
        let owned: Vec<String> = texts.iter().map(|t| (*t).to_owned()).collect();

        tokio::task::spawn_blocking(move || {
            let mut guard = model.lock().map_err(|e| {
                EmbedError::InternalError(format!("model mutex poisoned: {e}"))
            })?;
            guard.embed(owned, None).map_err(|e| {
                EmbedError::InternalError(format!("batch embedding inference failed: {e}"))
            })
        })
        .await
        .map_err(|e| EmbedError::InternalError(format!("spawn_blocking join error: {e}")))?
    }

    fn dimensions(&self) -> u32 {
        DIMS
    }
}

// ---------------------------------------------------------------------------
// Validation helpers (testable without model download)
// ---------------------------------------------------------------------------

fn validate_text(text: &str) -> Result<(), EmbedError> {
    if text.is_empty() {
        return Err(EmbedError::InvalidInput(
            "text must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_text_at(text: &str, index: usize) -> Result<(), EmbedError> {
    if text.is_empty() {
        return Err(EmbedError::InvalidInput(format!(
            "text at index {index} must not be empty"
        )));
    }
    Ok(())
}

fn default_cache_dir() -> Result<PathBuf, EmbedError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| {
            EmbedError::ProviderUnavailable(
                "could not determine home directory (HOME / USERPROFILE not set)".to_string(),
            )
        })?;
    Ok(PathBuf::from(home).join(".cache/markymark/models"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Unit tests — no model download needed
    // -----------------------------------------------------------------------

    #[test]
    fn validate_text_rejects_empty() {
        let err = validate_text("").unwrap_err();
        assert!(matches!(err, EmbedError::InvalidInput(_)));
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn validate_text_accepts_non_empty() {
        assert!(validate_text("hello").is_ok());
    }

    #[test]
    fn validate_text_accepts_whitespace() {
        // Whitespace-only is technically valid (model handles tokenization).
        assert!(validate_text("   ").is_ok());
    }

    #[test]
    fn validate_text_at_rejects_empty_with_index() {
        let err = validate_text_at("", 3).unwrap_err();
        assert!(matches!(err, EmbedError::InvalidInput(_)));
        assert!(err.to_string().contains("index 3"));
    }

    #[test]
    fn validate_text_at_accepts_non_empty() {
        assert!(validate_text_at("hello", 0).is_ok());
    }

    #[test]
    fn default_cache_dir_under_home() {
        // HOME is always set in test environments.
        let dir = default_cache_dir().unwrap();
        assert!(dir.ends_with(".cache/markymark/models"));
    }

    #[test]
    fn dims_constant_is_384() {
        assert_eq!(DIMS, 384);
    }

    #[test]
    fn new_with_unwritable_cache_dir_returns_provider_unavailable() {
        // /dev/null is a file, not a directory — create_dir_all will fail.
        let result = LocalOnnxProvider::new(Some(PathBuf::from("/dev/null/nonexistent")));
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::ProviderUnavailable(msg) => {
                assert!(
                    msg.contains("cannot create cache directory"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected ProviderUnavailable, got: {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Integration tests — require model download (~90MB)
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_provider_new_succeeds() {
        let provider = LocalOnnxProvider::new(None).expect("provider should construct");
        assert_eq!(provider.dimensions(), 384);
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_returns_384_dimensions() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let embedding = provider.embed("hello world").await.unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_batch_returns_correct_count() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let results = provider
            .embed_batch(&["hello", "world", "test"])
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
        for vec in &results {
            assert_eq!(vec.len(), 384);
        }
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_empty_text_returns_invalid_input() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let result = provider.embed("").await;
        assert!(matches!(result.unwrap_err(), EmbedError::InvalidInput(_)));
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_batch_empty_slice_returns_empty_vec() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let results = provider.embed_batch(&[]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_unicode_text() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let result = provider.embed("こんにちは 🌍 مرحبا").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 384);
    }

    #[tokio::test]
    #[ignore = "requires ~90MB model download on first run"]
    async fn local_embed_long_text_truncates_gracefully() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        // all-MiniLM-L6-v2 has 256 token limit; fastembed truncates internally.
        let long_text = "word ".repeat(2000);
        let result = provider.embed(&long_text).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 384);
    }

    #[test]
    #[ignore = "requires ~90MB model download on first run"]
    fn debug_shows_model_name() {
        let provider = LocalOnnxProvider::new(None).unwrap();
        let debug = format!("{provider:?}");
        assert!(debug.contains("all-MiniLM-L6-v2"), "debug: {debug}");
        assert!(debug.contains("384"), "debug: {debug}");
    }
}
