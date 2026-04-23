//! Tests for `HashEmbeddingProvider` and its FNV-1a 32-bit hash backend.
//!
//! Gated on `semantic-search` at the module boundary (see `tests/mod.rs`);
//! per-test `#[cfg]` is intentionally absent.

use super::*;

/// fnv1a32 must produce the same u32 for the same bytes every time.
///
/// This pins the hash algorithm choice: `DefaultHasher` (SipHash 1-3) is
/// explicitly not stable across Rust versions per std docs.  FNV-1a 32-bit
/// is a fixed, well-specified algorithm that produces identical output
/// forever for the same input.
///
/// The constant 0x4f9f2cab is the standard FNV-1a 32-bit hash of "hello"
/// (verified against the reference implementation and online calculators).
#[tokio::test]
async fn fnv1a32_is_stable_and_deterministic() {
    let h1 = fnv1a32(b"hello");
    let h2 = fnv1a32(b"hello");
    assert_eq!(h1, h2, "same input must produce same hash");
    assert_ne!(
        fnv1a32(b"hello"),
        fnv1a32(b"world"),
        "distinct tokens must hash differently"
    );
    // Pin the exact value (verified against FNV-1a 32-bit reference implementation).
    assert_eq!(
        fnv1a32(b"hello"),
        0x4f9f2cab,
        "FNV-1a 32-bit hash of 'hello' must be 0x4f9f2cab"
    );
    // Empty string -> offset basis unchanged
    assert_eq!(
        fnv1a32(b""),
        0x811c9dc5,
        "empty bytes must return FNV offset basis"
    );
}

/// HashEmbeddingProvider must produce a normalized output vector of the
/// expected dimensionality.
#[tokio::test]
async fn hash_embedding_output_is_normalized_and_correct_dims() {
    let provider = HashEmbeddingProvider::new(128);
    let emb = provider.embed("hello world").await.unwrap();
    assert_eq!(emb.len(), 128, "embedding length must match dims");
    let norm_sq: f32 = emb.iter().map(|v| v * v).sum();
    assert!(
        (norm_sq - 1.0).abs() < 1e-5,
        "embedding must be L2-normalised, got norm^2={norm_sq}"
    );
}

/// HashEmbeddingProvider must produce identical vectors for identical input.
/// This test detects accidental use of randomised hashing (e.g. RandomState).
#[tokio::test]
async fn hash_embedding_is_deterministic() {
    let provider = HashEmbeddingProvider::new(64);
    let a = provider.embed("markymark semantic search").await.unwrap();
    let b = provider.embed("markymark semantic search").await.unwrap();
    assert_eq!(a, b, "identical input must produce identical embedding");
}

/// Empty text must fail with InvalidInput (not panic).
#[tokio::test]
async fn hash_embedding_rejects_empty_text() {
    let provider = HashEmbeddingProvider::new(32);
    let err = provider.embed("   ").await.unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "whitespace-only input must return InvalidInput, got {err:?}"
    );
}

/// Zero dims must fail with InvalidInput (not divide-by-zero).
#[tokio::test]
async fn hash_embedding_rejects_zero_dims() {
    let provider = HashEmbeddingProvider::new(0);
    let err = provider.embed("hello").await.unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "zero dims must return InvalidInput, got {err:?}"
    );
}
