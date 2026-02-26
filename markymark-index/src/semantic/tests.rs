    use super::*;
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
            sem.doc_to_ids.get(&uri).is_none(),
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
        assert_eq!(sem.entry_count(), 3, "all metadata entries should be committed");
        assert_eq!(sem.index.count(), 3, "all vectors should be committed to Zig");
        assert_eq!(
            sem.doc_to_ids.get(&uri).map(std::vec::Vec::len),
            Some(3),
            "doc_to_ids should track all committed ids",
        );
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
