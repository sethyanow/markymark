//! Inference provider trait for LLM-powered text operations.
//!
//! [`InferenceProvider`] provides a source-agnostic interface for generating
//! text completions (summaries, descriptions). The provider decision
//! (Anthropic, OpenAI, local) is deferred to implementation time.

use async_trait::async_trait;
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by inference operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceError {
    /// Invalid or empty input.
    InvalidInput(String),
    /// The inference provider is unavailable (network, auth, rate-limit).
    ProviderUnavailable(String),
    /// The provider returned an unexpected or malformed response.
    BadResponse(String),
    /// Internal inference failure.
    InternalError(String),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "inference: invalid input: {msg}"),
            Self::ProviderUnavailable(msg) => {
                write!(f, "inference: provider unavailable: {msg}")
            }
            Self::BadResponse(msg) => write!(f, "inference: bad response: {msg}"),
            Self::InternalError(msg) => write!(f, "inference: internal error: {msg}"),
        }
    }
}

impl std::error::Error for InferenceError {}

// ---------------------------------------------------------------------------
// InferenceProvider trait
// ---------------------------------------------------------------------------

/// Source-agnostic interface for LLM-powered text operations.
///
/// Implementations must be `Send + Sync` and object-safe.
#[async_trait]
pub trait InferenceProvider: Send + Sync {
    /// Summarize a text section, optionally with surrounding context.
    ///
    /// `text` is the section content to summarize.
    /// `context` is optional surrounding context (e.g. parent heading path,
    /// document title) to improve summary quality.
    async fn summarize(&self, text: &str, context: Option<&str>) -> Result<String, InferenceError>;

    /// Summarize multiple text sections in batch.
    ///
    /// Default implementation calls [`summarize`](Self::summarize) sequentially.
    /// Implementations may override for batch-optimized providers.
    async fn summarize_batch(
        &self,
        items: &[(&str, Option<&str>)],
    ) -> Result<Vec<String>, InferenceError> {
        let mut results = Vec::with_capacity(items.len());
        for (text, context) in items {
            results.push(self.summarize(text, *context).await?);
        }
        Ok(results)
    }

    /// Return the model identifier used by this provider (e.g. "claude-haiku-4-5").
    fn model_id(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MockInferenceProvider {
        model: String,
    }

    impl MockInferenceProvider {
        fn new(model: &str) -> Self {
            Self {
                model: model.to_string(),
            }
        }
    }

    #[async_trait]
    impl InferenceProvider for MockInferenceProvider {
        async fn summarize(
            &self,
            text: &str,
            context: Option<&str>,
        ) -> Result<String, InferenceError> {
            if text.is_empty() {
                return Err(InferenceError::InvalidInput("empty text".to_string()));
            }
            let prefix = context.unwrap_or("no context");
            Ok(format!(
                "[{prefix}] summary of: {}",
                &text[..text.len().min(30)]
            ))
        }

        fn model_id(&self) -> &str {
            &self.model
        }
    }

    #[tokio::test]
    async fn test_inference_provider_trait_object() {
        // Verifies InferenceProvider is object-safe (dyn-compatible).
        let provider: Box<dyn InferenceProvider> = Box::new(MockInferenceProvider::new("test"));
        let result = provider.summarize("hello world", None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("summary of:"));
    }

    #[test]
    fn test_inference_provider_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockInferenceProvider>();

        fn assert_dyn_send_sync(_: &(dyn InferenceProvider + Send + Sync)) {}
        let provider = MockInferenceProvider::new("test");
        assert_dyn_send_sync(&provider);
    }

    #[test]
    fn test_inference_provider_model_id() {
        let provider = MockInferenceProvider::new("claude-haiku-4-5");
        assert_eq!(provider.model_id(), "claude-haiku-4-5");
    }

    #[tokio::test]
    async fn test_inference_provider_summarize_with_context() {
        let provider = MockInferenceProvider::new("test");
        let result = provider
            .summarize("some text content", Some("# Parent > ## Section"))
            .await
            .unwrap();
        assert!(result.contains("# Parent > ## Section"));
        assert!(result.contains("summary of:"));
    }

    #[tokio::test]
    async fn test_inference_provider_summarize_empty_input() {
        let provider = MockInferenceProvider::new("test");
        let result = provider.summarize("", None).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            InferenceError::InvalidInput("empty text".to_string())
        );
    }

    #[tokio::test]
    async fn test_inference_provider_batch_default() {
        let provider = MockInferenceProvider::new("test");
        let items = vec![("first section", Some("ctx1")), ("second section", None)];
        let results = provider.summarize_batch(&items).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("ctx1"));
        assert!(results[1].contains("no context"));
    }

    #[tokio::test]
    async fn test_inference_provider_batch_propagates_error() {
        let provider = MockInferenceProvider::new("test");
        let items = vec![
            ("valid text", None),
            ("", None), // empty → error
        ];
        let result = provider.summarize_batch(&items).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_error_display() {
        let err = InferenceError::InvalidInput("empty".to_string());
        assert_eq!(err.to_string(), "inference: invalid input: empty");

        let err = InferenceError::ProviderUnavailable("offline".to_string());
        assert_eq!(err.to_string(), "inference: provider unavailable: offline");

        let err = InferenceError::BadResponse("malformed json".to_string());
        assert_eq!(err.to_string(), "inference: bad response: malformed json");

        let err = InferenceError::InternalError("oom".to_string());
        assert_eq!(err.to_string(), "inference: internal error: oom");
    }
}
