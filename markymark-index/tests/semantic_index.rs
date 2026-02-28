#![cfg(feature = "embeddings")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
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

#[derive(Debug)]
struct FailOnNthEmbeddingProvider {
    fail_on: usize,
    calls: AtomicUsize,
}

impl FailOnNthEmbeddingProvider {
    fn new(fail_on: usize) -> Self {
        Self {
            fail_on,
            calls: AtomicUsize::new(0),
        }
    }
}

impl EmbeddingProvider for FailOnNthEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(EmbedError::ProviderUnavailable(
                "provider unavailable".to_string(),
            ));
        }
        Ok(vec![1.0; 5])
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
fn test_add_document_failure_does_not_leave_partial_entries() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailOnNthEmbeddingProvider::new(2));
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(
            uri("partial.md"),
            &index_from("# First Heading\n\n## Second Heading"),
        )
        .expect_err("expected provider failure on second heading");
    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));

    assert_eq!(
        index.entry_count(),
        0,
        "failed add_document should not commit partial semantic entries",
    );
    let results = index
        .search("first", 10, 0.0)
        .expect("search after failed add_document should succeed");
    assert!(
        results.is_empty(),
        "failed add_document should not leak stale semantic matches",
    );
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

// --- Two-phase search_with_embedding tests (marky-qgg1 fix) ---

#[test]
fn test_search_with_embedding_matches_search() {
    // Verify search_with_embedding produces identical results to search() for the
    // same query, confirming the two-phase split doesn't change behaviour.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(Arc::clone(&provider)).expect("init");

    index
        .add_document(
            uri("rust.md"),
            &index_from("# Rust SIMD\n\nFast vectorized scanning."),
        )
        .expect("add rust");
    index
        .add_document(
            uri("graph.md"),
            &index_from("# Link Graph\n\nGraph based backlink traversal."),
        )
        .expect("add graph");

    let query = "rust simd";
    let embedding = provider.embed(query).expect("embed");

    let via_search = index
        .search(query, 5, 0.0)
        .expect("search should succeed");
    let via_embedding = index
        .search_with_embedding(&embedding, 5, 0.0)
        .expect("search_with_embedding should succeed");

    assert_eq!(
        via_search.len(),
        via_embedding.len(),
        "result counts must match"
    );
    for (a, b) in via_search.iter().zip(via_embedding.iter()) {
        assert_eq!(a.doc_uri, b.doc_uri, "doc_uris must match");
        assert_eq!(a.heading, b.heading, "headings must match");
    }
}

#[test]
fn test_provider_accessor_returns_cloneable_provider() {
    // Verify provider() returns a usable Arc clone for the two-phase pattern.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(Arc::clone(&provider)).expect("init");

    let cloned = index.provider();
    // The cloned provider should be callable and produce the same embeddings.
    let emb_original = provider.embed("rust simd").expect("embed original");
    let emb_cloned = cloned.embed("rust simd").expect("embed cloned");
    assert_eq!(
        emb_original, emb_cloned,
        "cloned provider must produce identical embeddings"
    );
}

#[test]
fn test_search_with_embedding_empty_index_returns_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider.clone()).expect("init");

    let embedding = provider.embed("rust").expect("embed");
    let results = index
        .search_with_embedding(&embedding, 5, 0.0)
        .expect("search on empty index should succeed");
    assert!(results.is_empty());
}

#[test]
fn test_realm_embedding_provider_and_search_with_embedding() {
    // Verify the two-phase realm API works end-to-end.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut realm = RealmIndex::new_with_embeddings(provider).expect("realm with embeddings");

    let rust_uri = uri("rust-notes.md");
    realm.add_document(
        rust_uri.clone(),
        index_from("# Rust Notes\n\nSemantic search via embeddings"),
    );

    // Phase 1: get provider outside the lock.
    let p = realm
        .embedding_provider()
        .expect("embedding provider should be configured");

    // Phase 2: embed outside the lock.
    let embedding = p.embed("rust").expect("embed");

    // Phase 3: search inside the lock with pre-computed embedding.
    let results = realm
        .semantic_search_with_embedding(&embedding, 3, 0.0)
        .expect("search_with_embedding should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
}

#[test]
fn test_realm_embedding_provider_none_when_no_embeddings() {
    // When embeddings are not configured, embedding_provider() returns None.
    let realm = RealmIndex::new();
    assert!(
        realm.embedding_provider().is_none(),
        "no provider when embeddings not configured"
    );
}

#[test]
fn test_realm_semantic_search_with_embedding_none_returns_empty() {
    // When embeddings are not configured, search_with_embedding returns empty.
    let realm = RealmIndex::new();
    let embedding = vec![1.0_f32; 5];
    let results = realm
        .semantic_search_with_embedding(&embedding, 5, 0.0)
        .expect("should succeed with no semantic index");
    assert!(results.is_empty());
}
