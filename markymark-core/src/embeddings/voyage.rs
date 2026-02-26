//! Voyage AI embedding provider.
//!
//! Implements [`EmbeddingProvider`] using the [Voyage AI](https://voyageai.com) embeddings API.
//! Requires the `voyage` feature flag and a valid `VOYAGE_API_KEY`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{EmbedError, EmbeddingProvider};

/// Maximum number of texts per API request (conservative limit; API allows 1000).
const DEFAULT_BATCH_CHUNK_SIZE: usize = 128;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct VoyageRequest<'a> {
    input: Vec<&'a str>,
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimension: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageEmbeddingData>,
    #[allow(dead_code)]
    usage: VoyageUsage,
}

#[derive(Debug, Deserialize)]
struct VoyageEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct VoyageUsage {
    #[allow(dead_code)]
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct VoyageErrorResponse {
    detail: String,
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// Configuration for [`VoyageProvider`].
pub struct VoyageConfig {
    /// Voyage AI model name (default: `"voyage-3"`).
    pub model: String,
    /// Embedding dimensionality (default: 1024).
    pub dimensions: u32,
    /// API base URL (default: `"https://api.voyageai.com"`). Override for testing.
    pub base_url: String,
    /// Maximum texts per API request (default: 128).
    pub batch_chunk_size: usize,
}

impl Default for VoyageConfig {
    fn default() -> Self {
        Self {
            model: "voyage-3".to_string(),
            dimensions: 1024,
            base_url: "https://api.voyageai.com".to_string(),
            batch_chunk_size: DEFAULT_BATCH_CHUNK_SIZE,
        }
    }
}

/// Embedding provider backed by the Voyage AI API.
///
/// Uses async [`reqwest`] for HTTP and requires a valid API key.
/// API keys are never included in error messages or debug output.
pub struct VoyageProvider {
    client: reqwest::Client,
    api_key: String,
    config: VoyageConfig,
}

// Manual Debug to mask api_key
impl std::fmt::Debug for VoyageProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoyageProvider")
            .field("model", &self.config.model)
            .field("dimensions", &self.config.dimensions)
            .field("base_url", &self.config.base_url)
            .field("api_key", &"***")
            .finish()
    }
}

impl VoyageProvider {
    /// Create a new [`VoyageProvider`].
    ///
    /// Returns [`EmbedError::ProviderUnavailable`] if `api_key` is empty.
    pub fn new(api_key: String, config: VoyageConfig) -> Result<Self, EmbedError> {
        if api_key.trim().is_empty() {
            return Err(EmbedError::ProviderUnavailable(
                "VOYAGE_API_KEY is empty or not set".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| EmbedError::InternalError(format!("failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            api_key,
            config,
        })
    }

    /// Post an embedding request to the Voyage API and return parsed response.
    async fn post_embeddings(&self, texts: &[&str]) -> Result<VoyageResponse, EmbedError> {
        let url = format!("{}/v1/embeddings", self.config.base_url);

        let body = VoyageRequest {
            input: texts.to_vec(),
            model: &self.config.model,
            input_type: None,
            output_dimension: Some(self.config.dimensions),
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbedError::InternalError(format!("HTTP request failed: {e}")))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(EmbedError::ProviderUnavailable(
                "invalid or expired VOYAGE_API_KEY".to_string(),
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let detail = resp
                .json::<VoyageErrorResponse>()
                .await
                .map(|e| e.detail)
                .unwrap_or_else(|_| "rate limited".to_string());
            return Err(EmbedError::InternalError(format!("rate limited: {detail}")));
        }

        if !status.is_success() {
            let detail = resp
                .json::<VoyageErrorResponse>()
                .await
                .map(|e| e.detail)
                .unwrap_or_else(|_| format!("HTTP {status}"));
            return Err(EmbedError::InternalError(format!(
                "Voyage API error: {detail}"
            )));
        }

        let voyage_resp: VoyageResponse = resp.json().await.map_err(|e| {
            EmbedError::InternalError(format!("failed to parse Voyage API response: {e}"))
        })?;

        Ok(voyage_resp)
    }

    /// Validate that a single embedding vector has the expected dimensions.
    fn validate_dimensions(&self, vec: &[f32]) -> Result<(), EmbedError> {
        let actual = vec.len() as u32;
        if actual != self.config.dimensions {
            return Err(EmbedError::DimensionMismatch {
                expected: self.config.dimensions,
                actual,
            });
        }

        // Check for NaN/Infinity
        if vec.iter().any(|v| !v.is_finite()) {
            return Err(EmbedError::InternalError(
                "embedding contains NaN or Infinity values".to_string(),
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl EmbeddingProvider for VoyageProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        if text.is_empty() {
            return Err(EmbedError::InvalidInput(
                "text must not be empty".to_string(),
            ));
        }

        let resp = self.post_embeddings(&[text]).await?;

        let embedding = resp
            .data
            .into_iter()
            .next()
            .ok_or_else(|| {
                EmbedError::InternalError("Voyage API returned empty data array".to_string())
            })?
            .embedding;

        self.validate_dimensions(&embedding)?;
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Validate no empty strings
        for (i, t) in texts.iter().enumerate() {
            if t.is_empty() {
                return Err(EmbedError::InvalidInput(format!(
                    "text at index {i} must not be empty"
                )));
            }
        }

        let mut all_results: Vec<(usize, Vec<f32>)> = Vec::with_capacity(texts.len());
        let chunk_size = self.config.batch_chunk_size;

        for (chunk_idx, chunk) in texts.chunks(chunk_size).enumerate() {
            let resp = self.post_embeddings(chunk).await?;

            for item in resp.data {
                self.validate_dimensions(&item.embedding)?;
                // Global index = chunk offset + local index
                let global_idx = chunk_idx * chunk_size + item.index;
                all_results.push((global_idx, item.embedding));
            }
        }

        // Sort by index to respect API ordering
        all_results.sort_by_key(|(idx, _)| *idx);
        Ok(all_results.into_iter().map(|(_, vec)| vec).collect())
    }

    fn dimensions(&self) -> u32 {
        self.config.dimensions
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: build a VoyageProvider pointing at a wiremock server.
    async fn provider_for(server: &MockServer) -> VoyageProvider {
        VoyageProvider::new(
            "test-api-key".to_string(),
            VoyageConfig {
                base_url: server.uri(),
                dimensions: 1024,
                ..Default::default()
            },
        )
        .expect("provider creation should succeed")
    }

    /// Helper: build a mock 200 response with N embeddings of given dimension.
    fn ok_response(count: usize, dims: usize) -> serde_json::Value {
        let data: Vec<serde_json::Value> = (0..count)
            .map(|i| {
                serde_json::json!({
                    "object": "embedding",
                    "embedding": vec![0.1_f32; dims],
                    "index": i,
                })
            })
            .collect();

        serde_json::json!({
            "object": "list",
            "data": data,
            "model": "voyage-3",
            "usage": { "total_tokens": 42 }
        })
    }

    // -----------------------------------------------------------------------
    // Happy path
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_embed_single_text_returns_correct_dimensions() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("Authorization", "Bearer test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(1, 1024)))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello world").await.unwrap();
        assert_eq!(result.len(), 1024);
    }

    #[tokio::test]
    async fn test_embed_batch_within_limit() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(5, 1024)))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let texts: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let result = provider.embed_batch(&texts).await.unwrap();
        assert_eq!(result.len(), 5);
        for vec in &result {
            assert_eq!(vec.len(), 1024);
        }
    }

    #[tokio::test]
    async fn test_embed_batch_exceeding_chunk_size() {
        let server = MockServer::start().await;

        // Use small chunk size for testing
        let provider = VoyageProvider::new(
            "test-api-key".to_string(),
            VoyageConfig {
                base_url: server.uri(),
                dimensions: 4,
                batch_chunk_size: 3,
                ..Default::default()
            },
        )
        .unwrap();

        // First chunk: 3 items
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(3, 4)))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second chunk: 2 items
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(2, 4)))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        let texts: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let result = provider.embed_batch(&texts).await.unwrap();
        assert_eq!(result.len(), 5);
    }

    #[tokio::test]
    async fn test_dimensions_returns_configured_value() {
        let provider = VoyageProvider::new(
            "test-key".to_string(),
            VoyageConfig {
                dimensions: 512,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(provider.dimensions(), 512);
    }

    // -----------------------------------------------------------------------
    // Error handling
    // -----------------------------------------------------------------------

    #[test]
    fn test_embed_missing_api_key_returns_provider_unavailable() {
        let result = VoyageProvider::new(String::new(), VoyageConfig::default());
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::ProviderUnavailable(msg) => {
                assert!(msg.contains("VOYAGE_API_KEY"), "unexpected message: {msg}");
            }
            other => panic!("expected ProviderUnavailable, got: {other:?}"),
        }
    }

    #[test]
    fn test_embed_whitespace_api_key_returns_provider_unavailable() {
        let result = VoyageProvider::new("   ".to_string(), VoyageConfig::default());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbedError::ProviderUnavailable(_)
        ));
    }

    #[tokio::test]
    async fn test_embed_empty_text_returns_invalid_input() {
        let server = MockServer::start().await;
        let provider = provider_for(&server).await;
        let result = provider.embed("").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmbedError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_embed_http_401_returns_provider_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "detail": "invalid api key"
            })))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::ProviderUnavailable(msg) => {
                assert!(msg.contains("VOYAGE_API_KEY"), "unexpected message: {msg}");
            }
            other => panic!("expected ProviderUnavailable, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_embed_http_429_returns_rate_limit_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "detail": "too many requests"
            })))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::InternalError(msg) => {
                assert!(msg.contains("rate limited"), "unexpected message: {msg}");
            }
            other => panic!("expected InternalError with rate limit, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_embed_http_500_returns_internal_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "detail": "internal server error"
            })))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmbedError::InternalError(_)));
    }

    #[tokio::test]
    async fn test_embed_malformed_json_returns_internal_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::InternalError(msg) => {
                assert!(msg.contains("parse"), "unexpected message: {msg}");
            }
            other => panic!("expected InternalError for parse failure, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_embed_wrong_dimension_count_returns_dimension_mismatch() {
        let server = MockServer::start().await;
        // Return 512-dim vector when 1024 expected
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(1, 512)))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 512);
            }
            other => panic!("expected DimensionMismatch, got: {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_embed_batch_empty_slice_returns_ok_empty() {
        let server = MockServer::start().await;
        let provider = provider_for(&server).await;
        let result = provider.embed_batch(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_embed_batch_with_empty_string_returns_invalid_input() {
        let server = MockServer::start().await;
        let provider = provider_for(&server).await;
        let result = provider.embed_batch(&["hello", ""]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::InvalidInput(msg) => {
                assert!(msg.contains("index 1"), "unexpected message: {msg}");
            }
            other => panic!("expected InvalidInput, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_embed_response_with_nan_returns_internal_error() {
        let server = MockServer::start().await;

        // JSON spec doesn't support NaN, so serde rejects it at parse time.
        // Use a raw string body with "NaN" to simulate a non-standard API returning it.
        let raw_body = r#"{"object":"list","data":[{"object":"embedding","embedding":[NaN,0.1],"index":0}],"model":"voyage-3","usage":{"total_tokens":1}}"#;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(raw_body))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        let result = provider.embed("hello").await;
        assert!(result.is_err());
        // NaN in JSON causes a parse failure, caught as InternalError
        match result.unwrap_err() {
            EmbedError::InternalError(msg) => {
                assert!(msg.contains("parse"), "unexpected message: {msg}");
            }
            other => panic!("expected InternalError for NaN parse failure, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_embed_unicode_text_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_response(1, 1024)))
            .mount(&server)
            .await;

        let provider = provider_for(&server).await;
        // CJK, emoji, RTL
        let result = provider.embed("你好世界 🌍 مرحبا").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1024);
    }

    #[test]
    fn test_debug_does_not_leak_api_key() {
        let provider =
            VoyageProvider::new("super-secret-key".to_string(), VoyageConfig::default()).unwrap();
        let debug_output = format!("{provider:?}");
        assert!(
            !debug_output.contains("super-secret"),
            "Debug output leaked API key: {debug_output}"
        );
        assert!(debug_output.contains("***"));
    }
}
