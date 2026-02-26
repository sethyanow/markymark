//! Semantic index built on Zig embedding kernels.
//!
//! This module is feature-gated behind `embeddings`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use crate::DocumentIndex;
use markymark_core::prelude::*;
use markymark_kernels::embed::EmbeddingIndex as ZigEmbeddingIndex;

/// Semantic metadata for a heading-level search entry.
#[derive(Debug, Clone)]
pub struct SemanticEntry {
    /// Document URI containing this entry.
    pub doc_uri: DocumentUri,
    /// Heading text used as semantic label.
    pub heading: String,
    /// Markdown heading level (1-6).
    pub heading_level: u8,
    /// Section start position.
    pub section_start: Position,
    /// Section end position.
    pub section_end: Position,
}

/// Semantic search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matched document URI.
    pub doc_uri: DocumentUri,
    /// Matched heading text.
    pub heading: String,
    /// Matched heading level.
    pub heading_level: u8,
    /// Similarity score.
    pub score: f32,
    /// Source range for the matched heading/section.
    pub section_range: Range,
}

/// Pair of near-duplicate documents.
#[derive(Debug, Clone)]
pub struct DuplicateMatch {
    /// First URI in the pair.
    pub doc_uri_a: DocumentUri,
    /// Second URI in the pair.
    pub doc_uri_b: DocumentUri,
    /// Jaccard similarity over token hashes.
    pub similarity: f32,
}

/// Semantic index backed by [`ZigEmbeddingIndex`].
///
/// Stores entry metadata keyed by stable IDs and filters stale embedding IDs at
/// query time. This supports document replacement/removal even though the
/// current Zig embedding index API does not expose a delete operation.
pub struct SemanticIndex {
    provider: Arc<dyn EmbeddingProvider>,
    index: ZigEmbeddingIndex,
    entries_by_id: HashMap<String, SemanticEntry>,
    doc_to_ids: HashMap<DocumentUri, Vec<String>>,
    doc_token_sets: HashMap<DocumentUri, BTreeSet<u32>>,
}

const FETCH_OVERFETCH_MULTIPLIER: u32 = 4;

impl SemanticIndex {
    /// Create a semantic index using the provided embedding backend.
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Result<Self, EmbedError> {
        let dims = provider.dimensions();
        if dims == 0 {
            return Err(EmbedError::InvalidInput(
                "embedding dimensions must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            provider,
            index: ZigEmbeddingIndex::new(dims)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?,
            entries_by_id: HashMap::new(),
            doc_to_ids: HashMap::new(),
            doc_token_sets: HashMap::new(),
        })
    }

    /// Add (or replace) semantic entries for a document.
    ///
    /// If the document has headings, one semantic entry is generated per
    /// heading. If it has no headings, a single fallback entry based on the
    /// document file stem is created.
    pub async fn add_document(
        &mut self,
        uri: DocumentUri,
        index: &DocumentIndex,
    ) -> Result<(), EmbedError> {
        self.remove_document(&uri);

        let mut ids = Vec::new();
        let mut pending_entries = Vec::new();
        let mut token_set = BTreeSet::new();

        if index.headings().is_empty() {
            let fallback_heading = fallback_heading(&uri);
            let embedding = self.provider.embed(&fallback_heading).await?;
            let id = format!("{}#fallback", uri.as_str());
            self.index
                .add(&id, &embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;

            token_set.extend(token_hashes(&fallback_heading));
            pending_entries.push((
                id.clone(),
                SemanticEntry {
                    doc_uri: uri.clone(),
                    heading: fallback_heading,
                    heading_level: 1,
                    section_start: Position::new(0, 0),
                    section_end: Position::new(0, 0),
                },
            ));
            ids.push(id);
        } else {
            for (i, heading) in index.headings().iter().enumerate() {
                let embedding_input = heading.text.to_string();
                if embedding_input.trim().is_empty() {
                    continue;
                }
                let embedding = self.provider.embed(&embedding_input).await?;
                let id = format!("{}#{}#{i}", uri.as_str(), heading.slug);
                self.index
                    .add(&id, &embedding)
                    .map_err(|e| EmbedError::InternalError(e.to_string()))?;

                token_set.extend(token_hashes(&embedding_input));
                pending_entries.push((
                    id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: heading.text.to_string(),
                        heading_level: heading.level,
                        section_start: heading.range.start,
                        section_end: heading.range.end,
                    },
                ));
                ids.push(id);
            }
        }

        for (id, entry) in pending_entries {
            self.entries_by_id.insert(id, entry);
        }
        self.doc_to_ids.insert(uri.clone(), ids);
        self.doc_token_sets.insert(uri, token_set);
        Ok(())
    }

    /// Remove semantic metadata for a document.
    ///
    /// This removes in-memory metadata and duplicate-detection tokens. The
    /// underlying embedding vectors remain in the Zig index and are filtered out
    /// at query time by ID.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        if let Some(ids) = self.doc_to_ids.remove(uri) {
            for id in ids {
                self.entries_by_id.remove(&id);
            }
        }
        self.doc_token_sets.remove(uri);
    }

    /// Incrementally update semantic entries for a document.
    ///
    /// Diffs old vs new headings by **text** (not ID) and only re-embeds
    /// changed or added headings. Unchanged headings reuse their existing
    /// entries with updated metadata (level, range). Deleted headings have
    /// their metadata removed; stale Zig vectors are filtered at query time.
    ///
    /// If the provider fails mid-update, the old state is preserved (staged
    /// changes are committed only on full success).
    pub async fn update_document(
        &mut self,
        uri: DocumentUri,
        index: &DocumentIndex,
    ) -> Result<(), EmbedError> {
        // If no prior entries exist, delegate to add_document.
        let Some(old_ids) = self.doc_to_ids.get(&uri).cloned() else {
            return self.add_document(uri, index).await;
        };

        // Build map: heading_text → Vec<(entry_id, SemanticEntry)> from old entries.
        let mut old_by_text: HashMap<String, Vec<(String, SemanticEntry)>> = HashMap::new();
        for id in &old_ids {
            if let Some(entry) = self.entries_by_id.get(id) {
                old_by_text
                    .entry(entry.heading.clone())
                    .or_default()
                    .push((id.clone(), entry.clone()));
            }
        }

        // Build new heading list from index.
        let new_headings: Vec<_> = if index.headings().is_empty() {
            let fb = fallback_heading(&uri);
            vec![(fb, 1u8, Position::new(0, 0), Position::new(0, 0), true)]
        } else {
            index
                .headings()
                .iter()
                .filter(|h| !h.text.trim().is_empty())
                .map(|h| {
                    (
                        h.text.to_string(),
                        h.level,
                        h.range.start,
                        h.range.end,
                        false,
                    )
                })
                .collect()
        };

        // Check if old entries were a fallback.
        let old_was_fallback = old_ids.len() == 1
            && old_ids
                .first()
                .is_some_and(|id| id.ends_with("#fallback"));

        // Determine new_is_fallback.
        let new_is_fallback = new_headings.len() == 1 && new_headings[0].4;

        // If transitioning between fallback ↔ headings, delegate to full replace
        // since there's nothing to diff (different ID formats).
        if old_was_fallback != new_is_fallback {
            self.remove_document(&uri);
            return self.add_document(uri, index).await;
        }

        // Stage: collect changes to apply atomically.
        let mut new_ids = Vec::new();
        let mut staged_entries: Vec<(String, SemanticEntry)> = Vec::new();
        let mut staged_zig_adds: Vec<(String, Vec<f32>)> = Vec::new();
        let mut token_set = BTreeSet::new();

        // Track which old text entries have been consumed (for duplicate text handling).
        let mut consumed_by_text: HashMap<String, usize> = HashMap::new();

        for (text, level, start, end, is_fallback) in &new_headings {
            token_set.extend(token_hashes(text));

            // Try to match by text.
            let consumed_idx = consumed_by_text.entry(text.clone()).or_insert(0);
            let matched = old_by_text
                .get(text)
                .and_then(|entries| entries.get(*consumed_idx));

            if let Some((old_id, _old_entry)) = matched {
                // Reuse existing entry — keep OLD ID so the Zig vector remains
                // searchable, update metadata only, no re-embed.
                *consumed_idx += 1;

                staged_entries.push((
                    old_id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: text.clone(),
                        heading_level: *level,
                        section_start: *start,
                        section_end: *end,
                    },
                ));
                new_ids.push(old_id.clone());
            } else {
                // New or changed heading — needs embedding.
                let embedding = self.provider.embed(text).await?;

                let id = if *is_fallback {
                    format!("{}#fallback", uri.as_str())
                } else {
                    let slug = index
                        .headings()
                        .iter()
                        .find(|h| h.text == *text && h.range.start == *start)
                        .map(|h| h.slug)
                        .unwrap_or("unknown");
                    let idx = new_ids.len();
                    format!("{}#{}#{idx}", uri.as_str(), slug)
                };

                staged_zig_adds.push((id.clone(), embedding));
                staged_entries.push((
                    id.clone(),
                    SemanticEntry {
                        doc_uri: uri.clone(),
                        heading: text.clone(),
                        heading_level: *level,
                        section_start: *start,
                        section_end: *end,
                    },
                ));
                new_ids.push(id);
            }
        }

        // --- Commit phase (all embed calls succeeded) ---

        // Add new vectors to Zig index.
        for (id, embedding) in staged_zig_adds {
            self.index
                .add(&id, &embedding)
                .map_err(|e| EmbedError::InternalError(e.to_string()))?;
        }

        // Remove ALL old entries for this document.
        for id in &old_ids {
            self.entries_by_id.remove(id);
        }

        // Insert staged entries.
        for (id, entry) in staged_entries {
            self.entries_by_id.insert(id, entry);
        }

        // Update doc_to_ids and token sets.
        self.doc_to_ids.insert(uri.clone(), new_ids);
        self.doc_token_sets.insert(uri, token_set);

        Ok(())
    }

    /// Run semantic search over indexed entries.
    pub async fn search(
        &self,
        query: &str,
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        if top_k == 0 || self.entries_by_id.is_empty() {
            return Ok(Vec::new());
        }

        let query_embedding = self.provider.embed(query).await?;
        let score_floor = min_score.clamp(0.0, 1.0);

        let fetch_k = compute_fetch_k(self.index.count(), self.entries_by_id.len() as u32, top_k);
        let raw = self
            .index
            .search(&query_embedding, fetch_k)
            .map_err(|e| EmbedError::InternalError(e.to_string()))?;

        let mut out = Vec::new();
        for candidate in raw {
            if candidate.score < score_floor {
                continue;
            }
            let Some(entry) = self.entries_by_id.get(&candidate.id) else {
                continue;
            };

            out.push(SearchResult {
                doc_uri: entry.doc_uri.clone(),
                heading: entry.heading.clone(),
                heading_level: entry.heading_level,
                score: candidate.score,
                section_range: Range::new(entry.section_start, entry.section_end),
            });

            if out.len() as u32 >= top_k {
                break;
            }
        }

        Ok(out)
    }

    /// Detect near-duplicate document pairs using token-hash Jaccard similarity.
    pub fn detect_duplicates(&self, threshold: f32) -> Vec<DuplicateMatch> {
        let threshold = threshold.clamp(0.0, 1.0);
        if self.doc_token_sets.len() < 2 {
            return Vec::new();
        }

        let mut uris = self.doc_token_sets.keys().cloned().collect::<Vec<_>>();
        uris.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        let mut out = Vec::new();
        for i in 0..uris.len() {
            for j in (i + 1)..uris.len() {
                let a = &uris[i];
                let b = &uris[j];
                let Some(set_a) = self.doc_token_sets.get(a) else {
                    continue;
                };
                let Some(set_b) = self.doc_token_sets.get(b) else {
                    continue;
                };

                let similarity = jaccard_similarity(set_a, set_b);
                if similarity >= threshold {
                    out.push(DuplicateMatch {
                        doc_uri_a: a.clone(),
                        doc_uri_b: b.clone(),
                        similarity,
                    });
                }
            }
        }

        out.sort_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then_with(|| a.doc_uri_a.as_str().cmp(b.doc_uri_a.as_str()))
                .then_with(|| a.doc_uri_b.as_str().cmp(b.doc_uri_b.as_str()))
        });

        out
    }

    /// Number of active semantic entries.
    pub fn entry_count(&self) -> usize {
        self.entries_by_id.len()
    }
}

fn fallback_heading(uri: &DocumentUri) -> String {
    uri.to_file_path()
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(uri.as_str())
        .to_string()
}

fn token_hashes(text: &str) -> Vec<u32> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| fnv1a32(&token.to_ascii_lowercase()))
        .collect()
}

fn compute_fetch_k(index_count: u32, active_count: u32, top_k: u32) -> u32 {
    if index_count == 0 || top_k == 0 || active_count == 0 {
        return 0;
    }
    // Scale fetch size by stale ratio so enough active entries survive filtering.
    // If 80% of vectors are stale, we need ~5x raw hits per desired result.
    let stale_adjusted = ((top_k as u64 * index_count as u64) / active_count as u64) as u32;
    let baseline = top_k.saturating_mul(FETCH_OVERFETCH_MULTIPLIER);
    let needed = stale_adjusted.max(baseline);
    index_count.min(needed)
}

fn fnv1a32(text: &str) -> u32 {
    const OFFSET: u32 = 0x811c9dc5;
    const PRIME: u32 = 0x0100_0193;

    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn jaccard_similarity(a: &BTreeSet<u32>, b: &BTreeSet<u32>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }

    let intersection = a.intersection(b).count() as f32;
    let union = (a.len() + b.len()) as f32 - intersection;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
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
            self.count
                .store(0, std::sync::atomic::Ordering::SeqCst);
        }

        fn embed_count(&self) -> u32 {
            self.count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl EmbeddingProvider for CountingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
            self.count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
            let n = self.count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n >= self.fail_after {
                return Err(EmbedError::InternalError("injected failure".to_string()));
            }
            self.inner.embed(text).await
        }

        fn dimensions(&self) -> u32 {
            self.inner.dimensions()
        }
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

        assert_eq!(provider.embed_count(), 0, "unchanged headings should not re-embed");
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

        assert_eq!(provider.embed_count(), 1, "only changed heading should re-embed");
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

        assert_eq!(provider.embed_count(), 0, "no changed/new headings, zero embed calls");
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

        assert_eq!(provider.embed_count(), 0, "identical doc should have zero embed calls");
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
        assert_eq!(entry.heading_level, 3, "heading level should be updated to 3");
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

        assert!(result.is_err(), "update should return error on embed failure");
        // Old state must be preserved — entries should still reference Alpha and Beta.
        assert_eq!(sem.entry_count(), 2, "old entries must be preserved on failure");
    }
}
