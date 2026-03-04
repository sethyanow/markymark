#![cfg(feature = "embeddings")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
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

#[async_trait]
impl EmbeddingProvider for KeywordEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
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

#[async_trait]
impl EmbeddingProvider for FailingEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
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

#[async_trait]
impl EmbeddingProvider for FailOnNthEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        // Fail persistently from `fail_on` onwards, not just on one call.
        if call >= self.fail_on {
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

#[tokio::test]
async fn test_semantic_index_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let results = index
        .search("rust", 10, 0.0)
        .await
        .expect("search should succeed");
    assert!(results.is_empty());

    let duplicates = index.detect_duplicates(0.8);
    assert!(duplicates.is_empty());
}

#[tokio::test]
async fn test_add_document_and_search() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let rust_uri = uri("rust.md");
    index
        .add_document(
            rust_uri.clone(),
            &index_from("# Rust SIMD\n\nFast vectorized markdown scanning."),
        )
        .await
        .expect("add rust doc");

    let graph_uri = uri("graph.md");
    index
        .add_document(
            graph_uri.clone(),
            &index_from("# Link Graph\n\nGraph based backlink traversal."),
        )
        .await
        .expect("add graph doc");

    let results = index
        .search("rust simd search", 5, 0.0)
        .await
        .expect("search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
    assert!(results[0].score >= results.last().expect("non-empty").score);
}

#[tokio::test]
async fn test_add_document_no_headings_uses_fallback() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let notes_uri = uri("notes.md");
    index
        .add_document(
            notes_uri.clone(),
            &index_from("plain content without headings"),
        )
        .await
        .expect("fallback indexing should succeed");

    let results = index
        .search("notes", 5, 0.0)
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, notes_uri);
}

#[tokio::test]
async fn test_embedding_provider_failure_propagates() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailingEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(uri("broken.md"), &index_from("# Broken"))
        .await
        .expect_err("expected provider failure");

    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));
}

#[tokio::test]
async fn test_add_document_failure_does_not_leave_partial_entries() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailOnNthEmbeddingProvider::new(2));
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(
            uri("partial.md"),
            &index_from("# First Heading\n\n## Second Heading"),
        )
        .await
        .expect_err("expected provider failure on second heading");
    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));

    assert_eq!(
        index.entry_count(),
        0,
        "failed add_document should not commit partial semantic entries",
    );
    // Use search_with_embedding to bypass the broken provider — we only want
    // to verify no stale entries leaked into the index.
    let dummy_embedding = vec![1.0_f32; 5];
    let results = index
        .search_with_embedding(&dummy_embedding, 10, 0.0)
        .expect("search after failed add_document should succeed");
    assert!(
        results.is_empty(),
        "failed add_document should not leak stale semantic matches",
    );
}

#[tokio::test]
async fn test_detect_duplicates_threshold() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    index
        .add_document(
            uri("doc-a.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .await
        .expect("add doc-a");
    index
        .add_document(
            uri("doc-b.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .await
        .expect("add doc-b");

    let duplicates = index.detect_duplicates(0.8);
    assert_eq!(duplicates.len(), 1);
    assert!(duplicates[0].similarity >= 0.8);
}

#[tokio::test]
async fn test_realm_semantic_search_integration() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut realm = RealmIndex::new_with_embeddings(provider).expect("realm with embeddings");

    let rust_uri = uri("r.md");
    realm
        .add_document(
            rust_uri.clone(),
            index_from("# Rust Search\n\nSemantic search via embeddings"),
        )
        .await;

    let results = realm
        .semantic_search("rust", 3, 0.0)
        .await
        .expect("semantic search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
}

// --- Two-phase search_with_embedding tests (marky-qgg1 fix) ---

#[tokio::test]
async fn test_search_with_embedding_matches_search() {
    // Verify search_with_embedding produces identical results to search() for the
    // same query, confirming the two-phase split doesn't change behaviour.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(Arc::clone(&provider)).expect("init");

    index
        .add_document(
            uri("rust.md"),
            &index_from("# Rust SIMD\n\nFast vectorized scanning."),
        )
        .await
        .expect("add rust");
    index
        .add_document(
            uri("graph.md"),
            &index_from("# Link Graph\n\nGraph based backlink traversal."),
        )
        .await
        .expect("add graph");

    let query = "rust simd";
    let embedding = provider.embed(query).await.expect("embed");

    let via_search = index
        .search(query, 5, 0.0)
        .await
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

#[tokio::test]
async fn test_provider_accessor_returns_cloneable_provider() {
    // Verify provider() returns a usable Arc clone for the two-phase pattern.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(Arc::clone(&provider)).expect("init");

    let cloned = index.provider();
    // The cloned provider should be callable and produce the same embeddings.
    let emb_original = provider.embed("rust simd").await.expect("embed original");
    let emb_cloned = cloned.embed("rust simd").await.expect("embed cloned");
    assert_eq!(
        emb_original, emb_cloned,
        "cloned provider must produce identical embeddings"
    );
}

#[test]
fn test_search_with_embedding_empty_index_returns_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider.clone()).expect("init");

    // Use a zeroed embedding since no docs are indexed.
    let embedding = vec![0.0_f32; 5];
    let results = index
        .search_with_embedding(&embedding, 5, 0.0)
        .expect("search on empty index should succeed");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_realm_two_phase_semantic_search() {
    // Verify the two-phase realm API works end-to-end via semantic_index_arc().
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut realm = RealmIndex::new_with_embeddings(provider).expect("realm with embeddings");

    let rust_uri = uri("rust-notes.md");
    realm
        .add_document(
            rust_uri.clone(),
            index_from("# Rust Notes\n\nSemantic search via embeddings"),
        )
        .await;

    // Phase 1: get the semantic index Arc.
    let sem_arc = realm
        .semantic_index_arc()
        .expect("semantic index should be configured");

    // Phase 2: lock briefly to get provider, then embed outside the lock.
    let embedding = {
        let guard = sem_arc.lock().await;
        let p = guard.provider();
        drop(guard); // release lock before embed
        p.embed("rust").await.expect("embed")
    };

    // Phase 3: search inside the lock with pre-computed embedding.
    let results = {
        let guard = sem_arc.lock().await;
        guard
            .search_with_embedding(&embedding, 3, 0.0)
            .expect("search_with_embedding should succeed")
    };

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
}

#[test]
fn test_realm_semantic_index_arc_none_when_no_embeddings() {
    // When embeddings are not configured, semantic_index_arc() returns None.
    let realm = RealmIndex::new();
    assert!(
        realm.semantic_index_arc().is_none(),
        "no arc when embeddings not configured"
    );
}

#[test]
fn test_search_with_embedding_zero_top_k_returns_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider).expect("init");
    let embedding = vec![0.0_f32; 5];
    let results = index
        .search_with_embedding(&embedding, 0, 0.0)
        .expect("zero top_k should succeed");
    assert!(results.is_empty());
}

// --- Blank headings fallback tests (marky-6pap fix) ---

#[tokio::test]
async fn test_add_document_blank_headings_uses_fallback() {
    // A document whose headings are ALL blank/whitespace should produce exactly
    // one fallback entry (identical behaviour to a document with no headings).
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let notes_uri = uri("notes.md");
    // "#   " and "##   " parse as headings with blank text.
    index
        .add_document(
            notes_uri.clone(),
            &index_from("#   \n\n##   \n\nsome body text about notes"),
        )
        .await
        .expect("add document with all-blank headings should succeed");

    assert_eq!(
        index.entry_count(),
        1,
        "all-blank headings should produce exactly one fallback entry"
    );

    // The fallback is keyed on the file stem ("notes"), so searching for
    // "notes" should surface it.
    let results = index
        .search("notes", 5, 0.0)
        .await
        .expect("search should succeed");
    assert!(
        !results.is_empty(),
        "document with all-blank headings must be searchable via fallback"
    );
    assert_eq!(results[0].doc_uri, notes_uri);
}

#[tokio::test]
async fn test_add_document_mixed_blank_and_real_headings() {
    // A document with one real heading + several blank headings should produce
    // exactly one entry (the real heading); no fallback is injected.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let rust_uri = uri("rust.md");
    index
        .add_document(
            rust_uri.clone(),
            &index_from("# Rust SIMD\n\n##   \n\n###   \n\ncontent"),
        )
        .await
        .expect("add document with mixed headings should succeed");

    assert_eq!(
        index.entry_count(),
        1,
        "only the real heading should produce an entry — no fallback"
    );

    let results = index
        .search("rust simd", 5, 0.0)
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
    assert_eq!(results[0].heading, "Rust SIMD");
}

#[tokio::test]
async fn test_update_document_all_blank_headings_fallback() {
    // Start with a real heading, then update so all headings become blank.
    // The updated document must produce a fallback entry and remain searchable.
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let notes_uri = uri("notes.md");

    // Initial add with a real heading.
    index
        .add_document(
            notes_uri.clone(),
            &index_from("# Notes Overview\n\ncontent"),
        )
        .await
        .expect("initial add should succeed");

    assert_eq!(index.entry_count(), 1, "one entry after initial add");

    // Update with all-blank headings.
    index
        .update_document(
            notes_uri.clone(),
            &index_from("#   \n\n##   \n\nupdated body about notes"),
        )
        .await
        .expect("update with all-blank headings should succeed");

    assert_eq!(
        index.entry_count(),
        1,
        "all-blank headings update must produce exactly one fallback entry"
    );

    let results = index
        .search("notes", 5, 0.0)
        .await
        .expect("search should succeed");
    assert!(
        !results.is_empty(),
        "document after all-blank headings update must be searchable via fallback"
    );
    assert_eq!(results[0].doc_uri, notes_uri);
}

// --- ID collision regression tests (marky-ce9o) ---

/// Two headings with the same text/slug exist in the old document.
/// Update replaces both with entirely new headings that share the same slug text.
/// All new entries must receive distinct IDs — no silent overwrite via HashMap collision.
#[tokio::test]
async fn test_update_document_id_collision_same_slug() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(Arc::clone(&provider)).expect("init");
    let doc_uri = uri("collision.md");

    // Initial: two headings with the same text produce 2 entries.
    index
        .add_document(doc_uri.clone(), &index_from("# Notes\n\n## Notes\n"))
        .await
        .expect("add initial doc");
    assert_eq!(index.entry_count(), 2, "initial doc should have 2 entries");

    // Update: replace both headings with entirely new headings sharing the same slug.
    // Neither old heading matches the new text, so both are treated as new.
    index
        .update_document(doc_uri.clone(), &index_from("# Search\n\n## Search\n"))
        .await
        .expect("update doc");

    assert_eq!(
        index.entry_count(),
        2,
        "both same-slug new headings must get distinct IDs — no collision overwrite",
    );

    // Verify both entries are findable via search.
    let embedding = provider.embed("search").await.expect("embed query");
    let results = index
        .search_with_embedding(&embedding, 10, 0.0)
        .expect("search");
    assert_eq!(
        results.len(),
        2,
        "search must return both same-slug entries after update",
    );
}

/// Add a doc with heading "Rust" at line 0. Update reuses that heading
/// (matched by text) and also inserts a brand-new heading "Rust" further down.
/// The new heading must not collide with the reused heading's ID.
#[tokio::test]
async fn test_update_reused_and_new_same_slug() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(Arc::clone(&provider)).expect("init");
    let doc_uri = uri("reused-slug.md");

    // Add a doc with one "Rust" heading.
    index
        .add_document(doc_uri.clone(), &index_from("# Rust\n"))
        .await
        .expect("add initial doc");
    assert_eq!(index.entry_count(), 1, "initial doc should have 1 entry");

    // Update: keep "Rust" (reused, matched by text) and add a second "Rust" heading.
    // The reused heading keeps its old ID; the new heading must get a distinct one.
    index
        .update_document(doc_uri.clone(), &index_from("# Rust\n\n## Rust\n"))
        .await
        .expect("update doc");

    assert_eq!(
        index.entry_count(),
        2,
        "reused + new same-slug heading must produce 2 distinct entries",
    );

    // Both entries should be retrievable via search.
    let embedding = provider.embed("rust").await.expect("embed query");
    let results = index
        .search_with_embedding(&embedding, 10, 0.0)
        .expect("search");
    assert_eq!(
        results.len(),
        2,
        "search must return both headings when reused and new share the same slug",
    );
}

/// Replace ALL headings in a document with entirely new headings that share
/// the same slug text. Every new heading must receive a unique ID even when
/// the collision-avoidance loop must increment the suffix counter multiple times.
#[tokio::test]
async fn test_update_all_new_headings_same_slug() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(Arc::clone(&provider)).expect("init");
    let doc_uri = uri("all-new-slug.md");

    // Seed with a single heading that will be fully replaced.
    index
        .add_document(doc_uri.clone(), &index_from("# Graph\n"))
        .await
        .expect("add initial doc");
    assert_eq!(index.entry_count(), 1, "initial doc should have 1 entry");

    // Update replaces all headings with 3 new headings sharing the same slug.
    // None of these match the old "Graph" heading, so all 3 are treated as new.
    index
        .update_document(
            doc_uri.clone(),
            &index_from("# Search\n\n## Search\n\n### Search\n"),
        )
        .await
        .expect("update doc");

    assert_eq!(
        index.entry_count(),
        3,
        "all three same-slug new headings must get distinct IDs",
    );

    // All 3 entries must be findable.
    let embedding = provider.embed("search").await.expect("embed query");
    let results = index
        .search_with_embedding(&embedding, 10, 0.0)
        .expect("search");
    assert_eq!(
        results.len(),
        3,
        "search must return all three same-slug entries after full replacement",
    );
}
