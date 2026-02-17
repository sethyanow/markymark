#![cfg(feature = "embeddings")]

use std::path::PathBuf;
use std::sync::Arc;

use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex, SemanticIndex};
use markymark_parser::Parser;

fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}")))
}

fn index_from(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse");
    DocumentIndex::from_ast(ast)
}

#[derive(Debug)]
struct KeywordEmbeddingProvider;

impl EmbeddingProvider for KeywordEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let lower = text.to_ascii_lowercase();
        Ok(vec![
            if lower.contains("rust") { 1.0 } else { 0.0 },
            if lower.contains("simd") { 1.0 } else { 0.0 },
            if lower.contains("graph") { 1.0 } else { 0.0 },
            if lower.contains("search") { 1.0 } else { 0.0 },
            if lower.contains("notes") { 1.0 } else { 0.0 },
        ])
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[derive(Debug)]
struct FailingEmbeddingProvider;

impl EmbeddingProvider for FailingEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        Err(EmbedError::ProviderUnavailable(
            "provider unavailable".to_string(),
        ))
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[test]
fn test_semantic_index_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let results = index
        .search("rust", 10, 0.0)
        .expect("search should succeed");
    assert!(results.is_empty());

    let duplicates = index.detect_duplicates(0.8);
    assert!(duplicates.is_empty());
}

#[test]
fn test_add_document_and_search() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let rust_uri = uri("rust.md");
    index
        .add_document(
            rust_uri.clone(),
            &index_from("# Rust SIMD\n\nFast vectorized markdown scanning."),
        )
        .expect("add rust doc");

    let graph_uri = uri("graph.md");
    index
        .add_document(
            graph_uri.clone(),
            &index_from("# Link Graph\n\nGraph based backlink traversal."),
        )
        .expect("add graph doc");

    let results = index
        .search("rust simd search", 5, 0.0)
        .expect("search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
    assert!(results[0].score >= results.last().expect("non-empty").score);
}

#[test]
fn test_add_document_no_headings_uses_fallback() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let notes_uri = uri("notes.md");
    index
        .add_document(
            notes_uri.clone(),
            &index_from("plain content without headings"),
        )
        .expect("fallback indexing should succeed");

    let results = index
        .search("notes", 5, 0.0)
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, notes_uri);
}

#[test]
fn test_embedding_provider_failure_propagates() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailingEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(uri("broken.md"), &index_from("# Broken"))
        .expect_err("expected provider failure");

    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));
}

#[test]
fn test_detect_duplicates_threshold() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    index
        .add_document(
            uri("doc-a.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .expect("add doc-a");
    index
        .add_document(
            uri("doc-b.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .expect("add doc-b");

    let duplicates = index.detect_duplicates(0.8);
    assert_eq!(duplicates.len(), 1);
    assert!(duplicates[0].similarity >= 0.8);
}

#[test]
fn test_realm_semantic_search_integration() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut realm = RealmIndex::new_with_embeddings(provider).expect("realm with embeddings");

    let rust_uri = uri("r.md");
    realm.add_document(
        rust_uri.clone(),
        index_from("# Rust Search\n\nSemantic search via embeddings"),
    );

    let results = realm
        .semantic_search("rust", 3, 0.0)
        .expect("semantic search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
}
