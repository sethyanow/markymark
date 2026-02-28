use super::helpers::{compute_fetch_k, fnv1a32};
use super::*;
use crate::DocumentIndex;
use std::sync::Arc;

use async_trait::async_trait;
use markymark_core::prelude::EmbedError;

// --- compute_fetch_k unit tests ---

#[test]
fn compute_fetch_k_limits_overfetch_for_small_top_k() {
    // All active: stale_adjusted = 5*1000/1000 = 5, baseline = 20 → 20
    assert_eq!(compute_fetch_k(1_000, 1_000, 5), 20);
}

#[test]
fn compute_fetch_k_never_exceeds_index_count() {
    assert_eq!(compute_fetch_k(17, 17, 8), 17);
}

#[test]
fn compute_fetch_k_handles_empty_index() {
    assert_eq!(compute_fetch_k(0, 0, 8), 0);
}

#[test]
fn compute_fetch_k_zero_active_returns_zero() {
    assert_eq!(compute_fetch_k(100, 0, 5), 0);
}

#[test]
fn compute_fetch_k_scales_up_for_stale_vectors() {
    // 100 total, 20 active (80% stale), top_k=5
    // stale_adjusted = 5 * 100 / 20 = 25, baseline = 20 → 25
    assert_eq!(compute_fetch_k(100, 20, 5), 25);
}

#[test]
fn compute_fetch_k_heavily_stale_fetches_all() {
    // 100 total, 2 active (98% stale), top_k=5
    // stale_adjusted = 5 * 100 / 2 = 250, capped at index_count → 100
    assert_eq!(compute_fetch_k(100, 2, 5), 100);
}

// --- Helper: deterministic test embedding provider ---

struct TestEmbeddingProvider {
    dims: u32,
    /// When true, rejects empty/whitespace text (like HashEmbeddingProvider).
    reject_empty: bool,
}

impl TestEmbeddingProvider {
    fn new(dims: u32) -> Self {
        Self {
            dims,
            reject_empty: true,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for TestEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        if self.reject_empty && text.trim().is_empty() {
            return Err(EmbedError::InvalidInput("empty text rejected".to_string()));
        }
        // Simple bag-of-words hash embedding (mirrors HashEmbeddingProvider).
        let mut out = vec![0.0_f32; self.dims as usize];
        for token in text
            .trim()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
        {
            let idx = (fnv1a32(token) as usize) % out.len();
            out[idx] += 1.0;
        }
        let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut out {
                *v /= norm;
            }
        }
        Ok(out)
    }

    fn dimensions(&self) -> u32 {
        self.dims
    }
}

fn build_doc_index(markdown: &str) -> DocumentIndex {
    let mut parser = markymark_parser::Parser::new().unwrap();
    let ast = parser.parse(markdown).unwrap();
    DocumentIndex::from_ast(ast)
}

// --- P2: empty heading skip tests ---

#[tokio::test]
async fn add_document_skips_empty_headings() {
    let provider = Arc::new(TestEmbeddingProvider::new(32));
    let mut sem = SemanticIndex::new(provider).unwrap();

    // Tree-sitter parses "# \n" as a heading with empty content.
    let doc_idx = build_doc_index("# Introduction\n# \n## Conclusion\n");

    let uri = DocumentUri::from_file_path(&std::path::PathBuf::from("/test.md"));
    // Must succeed, not abort on empty headings.
    sem.add_document(uri.clone(), &doc_idx).await.unwrap();

    // The empty heading should be skipped; only valid headings indexed.
    assert!(
        sem.entry_count() >= 2,
        "expected at least 2 entries, got {}",
        sem.entry_count()
    );
}

#[tokio::test]
async fn add_document_no_headings_uses_fallback() {
    let provider = Arc::new(TestEmbeddingProvider::new(32));
    let mut sem = SemanticIndex::new(provider).unwrap();

    let doc_idx = build_doc_index("Just some text, no headings.\n");
    let uri = DocumentUri::from_file_path(&std::path::PathBuf::from("/plain.md"));
    sem.add_document(uri, &doc_idx).await.unwrap();
    assert_eq!(
        sem.entry_count(),
        1,
        "no-heading doc should get fallback entry"
    );
}

// --- CountingProvider: tracks embed() call count for update_document tests ---

struct CountingProvider {
    inner: TestEmbeddingProvider,
    count: std::sync::atomic::AtomicU32,
}

impl CountingProvider {
    fn new(dims: u32) -> Self {
        Self {
            inner: TestEmbeddingProvider::new(dims),
            count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn reset(&self) {
        self.count.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn embed_count(&self) -> u32 {
        self.count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl EmbeddingProvider for CountingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.embed(text).await
    }

    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
}

// --- FailingProvider: fails after N successful embed calls ---

struct FailingProvider {
    inner: TestEmbeddingProvider,
    count: std::sync::atomic::AtomicU32,
    fail_after: u32,
}

impl FailingProvider {
    fn new(dims: u32, fail_after: u32) -> Self {
        Self {
            inner: TestEmbeddingProvider::new(dims),
            count: std::sync::atomic::AtomicU32::new(0),
            fail_after,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for FailingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let n = self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if n >= self.fail_after {
            return Err(EmbedError::InternalError("injected failure".to_string()));
        }
        self.inner.embed(text).await
    }

    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
}

struct BatchCountingProvider {
    inner: TestEmbeddingProvider,
    batch_count: std::sync::atomic::AtomicU32,
    embed_count: std::sync::atomic::AtomicU32,
}

impl BatchCountingProvider {
    fn new(dims: u32) -> Self {
        Self {
            inner: TestEmbeddingProvider::new(dims),
            batch_count: std::sync::atomic::AtomicU32::new(0),
            embed_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn batch_count(&self) -> u32 {
        self.batch_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn embed_count(&self) -> u32 {
        self.embed_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl EmbeddingProvider for BatchCountingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.embed_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.embed(text).await
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.batch_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.inner.embed(text).await?);
        }
        Ok(out)
    }

    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
}

struct BatchFailingProvider {
    inner: TestEmbeddingProvider,
    batch_count: std::sync::atomic::AtomicU32,
    embed_count: std::sync::atomic::AtomicU32,
}

impl BatchFailingProvider {
    fn new(dims: u32) -> Self {
        Self {
            inner: TestEmbeddingProvider::new(dims),
            batch_count: std::sync::atomic::AtomicU32::new(0),
            embed_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn batch_count(&self) -> u32 {
        self.batch_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn embed_count(&self) -> u32 {
        self.embed_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl EmbeddingProvider for BatchFailingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.embed_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.embed(text).await
    }

    async fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.batch_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(EmbedError::InternalError(
            "injected batch failure".to_string(),
        ))
    }

    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
}

#[tokio::test]
async fn test_add_document_partial_embed_failure_does_not_mutate_zig_index() {
    let provider = Arc::new(FailingProvider::new(32, 1));
    let mut sem = SemanticIndex::new(provider).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n");
    let result = sem.add_document(uri.clone(), &doc).await;
    assert!(result.is_err(), "expected injected embed failure");

    assert_eq!(sem.entry_count(), 0, "metadata should remain empty");
    assert_eq!(
        sem.index.count(),
        0,
        "no Zig vectors should be inserted when embeds fail",
    );
    assert!(
        !sem.doc_to_ids.contains_key(&uri),
        "doc_to_ids should not contain failed document",
    );
}

#[tokio::test]
async fn test_add_document_success_commits_all_vectors_and_metadata() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();

    assert_eq!(provider.embed_count(), 3, "all headings should be embedded");
    assert_eq!(
        sem.entry_count(),
        3,
        "all metadata entries should be committed"
    );
    assert_eq!(
        sem.index.count(),
        3,
        "all vectors should be committed to Zig"
    );
    assert_eq!(
        sem.doc_to_ids.get(&uri).map(std::vec::Vec::len),
        Some(3),
        "doc_to_ids should track all committed ids",
    );
}

#[tokio::test]
async fn test_add_documents_batch_matches_sequential_search_results() {
    let mut sequential = SemanticIndex::new(Arc::new(TestEmbeddingProvider::new(32))).unwrap();
    let batch_provider = Arc::new(BatchCountingProvider::new(32));
    let mut batched = SemanticIndex::new(batch_provider.clone()).unwrap();

    let uri_a = DocumentUri::from_file_path(&std::path::PathBuf::from("/a.md"));
    let uri_b = DocumentUri::from_file_path(&std::path::PathBuf::from("/b.md"));

    let doc_a = build_doc_index("# Alpha\n## Borrow Checker\n");
    let doc_b = build_doc_index("# Beta\n## Async Runtime\n");

    sequential
        .add_document(uri_a.clone(), &doc_a)
        .await
        .unwrap();
    sequential
        .add_document(uri_b.clone(), &doc_b)
        .await
        .unwrap();

    batched
        .add_documents(vec![(uri_a.clone(), &doc_a), (uri_b.clone(), &doc_b)])
        .await
        .unwrap();

    assert_eq!(
        batch_provider.batch_count(),
        1,
        "all headings across documents should use one batch request"
    );
    assert_eq!(
        batch_provider.embed_count(),
        0,
        "no sequential fallback should run on successful batch"
    );

    let seq_results = sequential.search("borrow async", 8, 0.0).await.unwrap();
    let batch_results = batched.search("borrow async", 8, 0.0).await.unwrap();

    assert_eq!(seq_results.len(), batch_results.len());
    for (seq, batch) in seq_results.iter().zip(batch_results.iter()) {
        assert_eq!(seq.doc_uri, batch.doc_uri);
        assert_eq!(seq.heading, batch.heading);
        assert_eq!(seq.heading_level, batch.heading_level);
        assert!(
            (seq.score - batch.score).abs() < 1e-6,
            "score mismatch for heading {}",
            seq.heading
        );
    }
}

#[tokio::test]
async fn test_add_documents_skips_empty_headings_in_batch_mode() {
    let provider = Arc::new(BatchCountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();
    let doc = build_doc_index("# Intro\n# \n## Conclusion\n");

    sem.add_documents(vec![(uri.clone(), &doc)]).await.unwrap();

    assert_eq!(provider.batch_count(), 1, "batch path should be used");
    assert_eq!(sem.entry_count(), 2, "empty heading should be skipped");
    assert_eq!(sem.doc_to_ids.get(&uri).map(std::vec::Vec::len), Some(2));
}

#[tokio::test]
async fn test_add_documents_mixed_docs_with_and_without_headings() {
    let provider = Arc::new(BatchCountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();

    let uri_a = DocumentUri::from_file_path(&std::path::PathBuf::from("/mix-a.md"));
    let uri_b = DocumentUri::from_file_path(&std::path::PathBuf::from("/mix-b.md"));
    let uri_c = DocumentUri::from_file_path(&std::path::PathBuf::from("/mix-c.md"));

    let doc_a = build_doc_index("# Alpha\n## Beta\n");
    let doc_b = build_doc_index("plain text without headings\n");
    let doc_c = build_doc_index("# \n## Gamma\n");

    sem.add_documents(vec![
        (uri_a.clone(), &doc_a),
        (uri_b.clone(), &doc_b),
        (uri_c.clone(), &doc_c),
    ])
    .await
    .unwrap();

    assert_eq!(
        provider.batch_count(),
        1,
        "all docs should be batched together"
    );
    assert_eq!(sem.entry_count(), 4, "2 + 1 fallback + 1 non-empty heading");
    assert_eq!(sem.doc_to_ids.get(&uri_a).map(std::vec::Vec::len), Some(2));
    assert_eq!(sem.doc_to_ids.get(&uri_b).map(std::vec::Vec::len), Some(1));
    assert_eq!(sem.doc_to_ids.get(&uri_c).map(std::vec::Vec::len), Some(1));

    let fallback_ids = sem.doc_to_ids.get(&uri_b).unwrap();
    assert!(
        fallback_ids[0].ends_with("#fallback"),
        "document without headings should use fallback entry"
    );
}

#[tokio::test]
async fn test_add_documents_batch_failure_falls_back_to_sequential_embed() {
    let provider = Arc::new(BatchFailingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();
    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");

    sem.add_documents(vec![(uri.clone(), &doc)]).await.unwrap();

    assert_eq!(
        provider.batch_count(),
        1,
        "batch path should be attempted first"
    );
    assert_eq!(
        provider.embed_count(),
        3,
        "sequential fallback should embed each non-empty heading"
    );
    assert_eq!(sem.entry_count(), 3);
    assert_eq!(sem.index.count(), 3);
}

// --- update_document tests ---

fn test_uri() -> DocumentUri {
    DocumentUri::from_file_path(&std::path::PathBuf::from("/test.md"))
}

#[tokio::test]
async fn test_update_unchanged_headings_skips_reembed() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 3);

    provider.reset();
    let same_doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.update_document(uri, &same_doc).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        0,
        "unchanged headings should not re-embed"
    );
    assert_eq!(sem.entry_count(), 3);
}

#[tokio::test]
async fn test_update_changed_heading_reembeds_only_changed() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();

    provider.reset();
    let updated = build_doc_index("# Alpha\n## BetaModified\n## Gamma\n");
    sem.update_document(uri, &updated).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        1,
        "only changed heading should re-embed"
    );
    assert_eq!(sem.entry_count(), 3);
}

#[tokio::test]
async fn test_update_added_heading_embeds_new_only() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 2);

    provider.reset();
    let updated = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.update_document(uri, &updated).await.unwrap();

    assert_eq!(provider.embed_count(), 1, "only new heading should embed");
    assert_eq!(sem.entry_count(), 3);
}

#[tokio::test]
async fn test_update_deleted_heading_removes_metadata() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 3);

    provider.reset();
    let updated = build_doc_index("# Alpha\n## Gamma\n");
    sem.update_document(uri, &updated).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        0,
        "no changed/new headings, zero embed calls"
    );
    assert_eq!(sem.entry_count(), 2, "deleted heading metadata removed");
}

#[tokio::test]
async fn test_update_no_changes_zero_embed_calls() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();

    provider.reset();
    let same = build_doc_index("# Alpha\n## Beta\n");
    sem.update_document(uri, &same).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        0,
        "identical doc should have zero embed calls"
    );
    assert_eq!(sem.entry_count(), 2);
}

#[tokio::test]
async fn test_update_fallback_to_headings_reembeds() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    // Start with no headings (fallback entry).
    let doc = build_doc_index("Just text, no headings.\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 1);

    provider.reset();
    // Update to a doc with headings.
    let updated = build_doc_index("# Alpha\n## Beta\n");
    sem.update_document(uri, &updated).await.unwrap();

    assert_eq!(provider.embed_count(), 2, "new headings replace fallback");
    assert_eq!(sem.entry_count(), 2, "fallback removed, headings added");
}

#[tokio::test]
async fn test_update_headings_to_fallback_reembeds() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 2);

    provider.reset();
    // Update to doc with no headings (becomes fallback).
    let updated = build_doc_index("Just text, no headings.\n");
    sem.update_document(uri, &updated).await.unwrap();

    assert_eq!(provider.embed_count(), 1, "fallback entry must be embedded");
    assert_eq!(sem.entry_count(), 1, "headings replaced by fallback");
}

#[tokio::test]
async fn test_update_reordered_headings_skips_reembed() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();

    provider.reset();
    // Reorder: Gamma before Beta, same text.
    let reordered = build_doc_index("# Alpha\n## Gamma\n## Beta\n");
    sem.update_document(uri, &reordered).await.unwrap();

    assert_eq!(provider.embed_count(), 0, "reorder should not re-embed");
    assert_eq!(sem.entry_count(), 3);
}

#[tokio::test]
async fn test_update_heading_level_change_updates_metadata() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("## Foo\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();

    provider.reset();
    // Same text "Foo" but different level (### vs ##).
    let updated = build_doc_index("### Foo\n");
    sem.update_document(uri.clone(), &updated).await.unwrap();

    assert_eq!(provider.embed_count(), 0, "text unchanged, no re-embed");
    // Verify the entry's heading level was updated.
    let ids = sem.doc_to_ids.get(&uri).unwrap();
    assert_eq!(ids.len(), 1);
    let entry = sem.entries_by_id.get(&ids[0]).unwrap();
    assert_eq!(
        entry.heading_level, 3,
        "heading level should be updated to 3"
    );
}

#[tokio::test]
async fn test_update_provider_failure_leaves_old_state() {
    // Provider that allows initial add_document (2 embeds) but fails during update.
    let provider = Arc::new(FailingProvider::new(32, 2));
    let mut sem = SemanticIndex::new(provider).unwrap();
    let uri = test_uri();

    let doc = build_doc_index("# Alpha\n## Beta\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 2);

    // Update adds a new heading "Gamma" — this embed call will fail.
    let updated = build_doc_index("# Alpha\n## Beta\n## Gamma\n");
    let result = sem.update_document(uri.clone(), &updated).await;

    assert!(
        result.is_err(),
        "update should return error on embed failure"
    );
    // Old state must be preserved — entries should still reference Alpha and Beta.
    assert_eq!(
        sem.entry_count(),
        2,
        "old entries must be preserved on failure"
    );
}

#[tokio::test]
async fn test_fallback_transition_failure_preserves_state() {
    // Provider succeeds for initial add (1 fallback embed) but fails on transition.
    let provider = Arc::new(FailingProvider::new(32, 1));
    let mut sem = SemanticIndex::new(provider).unwrap();
    let uri = test_uri();

    // Start with no headings → fallback entry.
    let doc = build_doc_index("Just text, no headings.\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 1, "fallback entry should exist");

    // Verify the fallback entry is present.
    let old_ids = sem.doc_to_ids.get(&uri).cloned().unwrap();
    assert_eq!(old_ids.len(), 1);
    assert!(old_ids[0].ends_with("#fallback"));

    // Now update to a doc with headings — triggers fallback→headings transition.
    // The provider will fail (already used its 1 allowed embed), so add_document
    // inside the transition should fail.
    let updated = build_doc_index("# Alpha\n## Beta\n");
    let result = sem.update_document(uri.clone(), &updated).await;

    assert!(
        result.is_err(),
        "transition should propagate provider failure"
    );
    // Old fallback state must be restored.
    assert_eq!(
        sem.entry_count(),
        1,
        "old fallback entry must survive failed transition"
    );
    let restored_ids = sem.doc_to_ids.get(&uri).unwrap();
    assert_eq!(restored_ids.len(), 1, "doc_to_ids must be restored");
    assert!(
        restored_ids[0].ends_with("#fallback"),
        "restored entry should be the original fallback"
    );
}

#[tokio::test]
async fn test_fallback_transition_success_replaces_state() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    // Start with no headings → fallback entry.
    let doc = build_doc_index("Just text, no headings.\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 1);

    let old_ids = sem.doc_to_ids.get(&uri).cloned().unwrap();
    assert!(old_ids[0].ends_with("#fallback"));

    provider.reset();
    // Update to a doc with headings — successful transition.
    let updated = build_doc_index("# Alpha\n## Beta\n");
    sem.update_document(uri.clone(), &updated).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        2,
        "both new headings should be embedded"
    );
    assert_eq!(sem.entry_count(), 2, "fallback replaced by 2 headings");

    // Verify no fallback entries remain.
    let new_ids = sem.doc_to_ids.get(&uri).unwrap();
    assert!(
        !new_ids.iter().any(|id| id.ends_with("#fallback")),
        "fallback entry should be gone after successful transition"
    );
}

// --- Regression tests: marky-6ri3 — non-atomic add_document loses entries on embed failure ---

/// add_document must rollback to the original entries when the embed provider
/// fails during re-indexing of an already-known document.
#[tokio::test]
async fn test_add_document_rollback_on_embed_failure() {
    // 4 dims; 1 embed call allowed (for initial add), 2nd call fails.
    let provider = Arc::new(FailingProvider::new(4, 1));
    let mut sem = SemanticIndex::new(provider).unwrap();
    let uri = DocumentUri::from_file_path(&std::path::PathBuf::from("/rollback.md"));

    // Initial add: 1 heading → 1 embed call (succeeds).
    let original = build_doc_index("# Original Heading\n");
    sem.add_document(uri.clone(), &original).await.unwrap();
    assert_eq!(sem.entry_count(), 1, "initial add should succeed");

    // Re-add with different headings — 1st embed call (count=1 >= 1) fails.
    let updated = build_doc_index("# Updated Heading\n");
    let result = sem.add_document(uri.clone(), &updated).await;

    assert!(result.is_err(), "re-add should fail due to provider failure");
    assert_eq!(
        sem.entry_count(),
        1,
        "rollback must preserve original entry count"
    );

    // Verify original heading text is still present in the index.
    let ids = sem.doc_to_ids.get(&uri).expect("doc_to_ids must be restored");
    assert_eq!(ids.len(), 1);
    let entry = sem.entries_by_id.get(&ids[0]).expect("entry must be restored");
    assert_eq!(
        entry.heading, "Original Heading",
        "rollback must restore original heading text"
    );
}

/// add_documents batch rollback: when the embed provider fails during a batch
/// re-index, ALL documents must be rolled back to their previous state.
#[tokio::test]
async fn test_add_documents_batch_rollback_on_failure() {
    // 4 dims; 2 embed calls allowed (1 per doc initial add), batch update fails.
    let provider = Arc::new(FailingProvider::new(4, 2));
    let mut sem = SemanticIndex::new(provider).unwrap();

    let uri_a = DocumentUri::from_file_path(&std::path::PathBuf::from("/batch-a.md"));
    let uri_b = DocumentUri::from_file_path(&std::path::PathBuf::from("/batch-b.md"));

    // Add doc A with 1 heading (1 embed call, count 0→1, succeeds).
    let doc_a = build_doc_index("# Alpha\n");
    sem.add_document(uri_a.clone(), &doc_a).await.unwrap();
    assert_eq!(sem.entry_count(), 1);

    // Add doc B with 1 heading (1 embed call, count 1→2, count=1 < 2, succeeds).
    let doc_b = build_doc_index("# Beta\n");
    sem.add_document(uri_b.clone(), &doc_b).await.unwrap();
    assert_eq!(sem.entry_count(), 2);

    // Batch update: provider is now at the limit (count=2 >= 2), first embed fails.
    let doc_a_updated = build_doc_index("# Alpha Updated\n");
    let doc_b_updated = build_doc_index("# Beta Updated\n");
    let result = sem
        .add_documents(vec![
            (uri_a.clone(), &doc_a_updated),
            (uri_b.clone(), &doc_b_updated),
        ])
        .await;

    assert!(result.is_err(), "batch update should fail");
    assert_eq!(
        sem.entry_count(),
        2,
        "both original entries must be restored after batch rollback"
    );

    // Verify both original headings are still present.
    let ids_a = sem
        .doc_to_ids
        .get(&uri_a)
        .expect("doc_a must be restored in doc_to_ids");
    let entry_a = sem
        .entries_by_id
        .get(&ids_a[0])
        .expect("doc_a entry must be restored");
    assert_eq!(
        entry_a.heading, "Alpha",
        "doc_a original heading must be preserved"
    );

    let ids_b = sem
        .doc_to_ids
        .get(&uri_b)
        .expect("doc_b must be restored in doc_to_ids");
    let entry_b = sem
        .entries_by_id
        .get(&ids_b[0])
        .expect("doc_b entry must be restored");
    assert_eq!(
        entry_b.heading, "Beta",
        "doc_b original heading must be preserved"
    );
}

/// add_document on a fresh (never-indexed) document that fails immediately must
/// leave the index in a clean empty state with no partial entries.
#[tokio::test]
async fn test_add_document_fresh_failure_leaves_clean_state() {
    // 4 dims; fail immediately on first embed call.
    let provider = Arc::new(FailingProvider::new(4, 0));
    let mut sem = SemanticIndex::new(provider).unwrap();
    let uri = DocumentUri::from_file_path(&std::path::PathBuf::from("/fresh-fail.md"));

    let doc = build_doc_index("# Fresh Heading\n");
    let result = sem.add_document(uri.clone(), &doc).await;

    assert!(result.is_err(), "add should fail immediately");
    assert_eq!(
        sem.entry_count(),
        0,
        "no partial state should remain after fresh add failure"
    );
    assert!(
        !sem.doc_to_ids.contains_key(&uri),
        "doc_to_ids must not contain the failed document"
    );
}

/// Regression: heading text "Foo!" and "Foo" produce the same slug but
/// different embedding text. `SemanticIndex::update_document` must detect
/// the text change and re-embed even though the slug is identical.
#[tokio::test]
async fn test_update_document_reembeds_on_text_change_same_slug() {
    let provider = Arc::new(CountingProvider::new(32));
    let mut sem = SemanticIndex::new(provider.clone()).unwrap();
    let uri = test_uri();

    // "Foo!" slugifies to "foo".
    let doc = build_doc_index("# Foo!\n");
    sem.add_document(uri.clone(), &doc).await.unwrap();
    assert_eq!(sem.entry_count(), 1);

    // Verify the initial heading text is "Foo!".
    let ids = sem.doc_to_ids.get(&uri).unwrap();
    assert_eq!(sem.entries_by_id.get(&ids[0]).unwrap().heading, "Foo!");

    provider.reset();

    // "Foo" also slugifies to "foo" — same slug, different text.
    let updated = build_doc_index("# Foo\n");
    sem.update_document(uri.clone(), &updated).await.unwrap();

    assert_eq!(
        provider.embed_count(),
        1,
        "text changed from 'Foo!' to 'Foo', must re-embed"
    );
    assert_eq!(sem.entry_count(), 1);

    // Verify the entry's heading text was updated.
    let ids = sem.doc_to_ids.get(&uri).unwrap();
    assert_eq!(
        sem.entries_by_id.get(&ids[0]).unwrap().heading,
        "Foo",
        "heading text should be updated from 'Foo!' to 'Foo'"
    );
}
