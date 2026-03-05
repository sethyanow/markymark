use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use crate::DocumentIndex;
use markymark_core::prelude::*;

use super::helpers::{
    build_embedding_input, compute_fetch_k, fallback_heading, jaccard_similarity,
    section_block_texts, token_hashes,
};
use super::{DuplicateMatch, SearchResult, SemanticEntry, SemanticIndex};

impl SemanticIndex {
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

        // Seed reserved_ids with all existing IDs to prevent collision when
        // new headings share a slug with reused or previously-assigned IDs.
        let mut reserved_ids: HashSet<String> = old_ids.iter().cloned().collect();

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

        // Build section text map for per-section embedding.
        let section_texts = section_block_texts(index);

        // Build new heading list from index.
        // Tuple: (text, level, start, end, is_fallback, heading_idx)
        let new_headings: Vec<_> = if index.headings().is_empty() {
            let fb = fallback_heading(&uri);
            vec![(
                fb,
                1u8,
                Position::new(0, 0),
                Position::new(0, 0),
                true,
                None,
            )]
        } else {
            let filtered: Vec<_> = index
                .headings()
                .iter()
                .enumerate()
                .filter(|(_, h)| !h.text.trim().is_empty())
                .map(|(i, h)| {
                    (
                        h.text.to_string(),
                        h.level,
                        h.range.start,
                        h.range.end,
                        false,
                        Some(i),
                    )
                })
                .collect();
            // Fallback: all headings were blank/whitespace — treat like no headings.
            if filtered.is_empty() {
                let fb = fallback_heading(&uri);
                vec![(
                    fb,
                    1u8,
                    Position::new(0, 0),
                    Position::new(0, 0),
                    true,
                    None,
                )]
            } else {
                filtered
            }
        };

        // Check if old entries were a fallback.
        let old_was_fallback =
            old_ids.len() == 1 && old_ids.first().is_some_and(|id| id.ends_with("#fallback"));

        // Determine new_is_fallback.
        let new_is_fallback = new_headings.len() == 1 && new_headings[0].4;

        // If transitioning between fallback ↔ headings, delegate to full replace
        // since there's nothing to diff (different ID formats).
        // Snapshot before remove so we can restore on failure.
        if old_was_fallback != new_is_fallback {
            let snapshot_entries: Vec<(String, SemanticEntry)> = old_ids
                .iter()
                .filter_map(|id| self.entries_by_id.get(id).cloned().map(|e| (id.clone(), e)))
                .collect();
            let snapshot_tokens = self.doc_token_sets.get(&uri).cloned();

            self.remove_document(&uri);
            match self.add_document(uri.clone(), index).await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    // Restore old state on failure.
                    for (id, entry) in snapshot_entries {
                        self.entries_by_id.insert(id, entry);
                    }
                    self.doc_to_ids.insert(uri.clone(), old_ids);
                    if let Some(tokens) = snapshot_tokens {
                        self.doc_token_sets.insert(uri, tokens);
                    }
                    return Err(err);
                }
            }
        }

        // Stage: collect changes to apply atomically.
        let mut new_ids = Vec::new();
        let mut staged_entries: Vec<(String, SemanticEntry)> = Vec::new();
        let mut staged_zig_adds: Vec<(String, Vec<f32>)> = Vec::new();
        let mut token_set = BTreeSet::new();

        // Track which old text entries have been consumed (for duplicate text handling).
        let mut consumed_by_text: HashMap<String, usize> = HashMap::new();

        for (text, level, start, end, is_fallback, heading_idx) in &new_headings {
            // Build per-section embedding input: heading + block text.
            let block_text = match heading_idx {
                Some(idx) => section_texts.get(&Some(*idx)).map(String::as_str),
                None if *is_fallback => section_texts.get(&None).map(String::as_str),
                _ => None,
            };
            let embedding_input = build_embedding_input(text, block_text);
            token_set.extend(token_hashes(&embedding_input));

            // Try to match by text.
            let consumed_idx = consumed_by_text.entry(text.clone()).or_insert(0);
            let matched = old_by_text
                .get(text)
                .and_then(|entries| entries.get(*consumed_idx));

            if let Some((old_id, _old_entry)) = matched {
                // Reuse existing entry — keep OLD ID so the Zig vector remains
                // searchable, update metadata only, no re-embed.
                *consumed_idx += 1;
                reserved_ids.insert(old_id.clone());

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
                let embedding = self.provider.embed(&embedding_input).await?;

                let id = if *is_fallback {
                    format!("{}#fallback", uri.as_str())
                } else {
                    let slug = index
                        .headings()
                        .iter()
                        .find(|h| h.text == *text && h.range.start == *start)
                        .map(|h| h.slug)
                        .unwrap_or("unknown");
                    let mut idx = new_ids.len();
                    loop {
                        let candidate = format!("{}#{}#{idx}", uri.as_str(), slug);
                        if reserved_ids.insert(candidate.clone()) {
                            break candidate;
                        }
                        idx += 1;
                    }
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

        // Add new vectors to Zig index (with rollback on partial failure).
        let mut added_ids: Vec<String> = Vec::new();
        for (id, embedding) in staged_zig_adds {
            match self.index.add(&id, &embedding) {
                Ok(()) => added_ids.push(id),
                Err(e) => {
                    for rollback_id in &added_ids {
                        let _ = self.index.remove(rollback_id);
                    }
                    return Err(EmbedError::InternalError(e.to_string()));
                }
            }
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

    /// Get a clone of the embedding provider.
    ///
    /// Callers that care about lock contention should clone the provider,
    /// call [`EmbeddingProvider::embed`] outside any lock, then call
    /// [`search_with_embedding`](Self::search_with_embedding) inside the lock.
    pub fn provider(&self) -> Arc<dyn EmbeddingProvider> {
        self.provider.clone()
    }

    /// Run semantic search over indexed entries.
    ///
    /// This embeds `query` via the provider and then performs the in-memory
    /// index search. If the caller holds a lock (e.g., a `TokioMutex`),
    /// consider using [`provider`](Self::provider) +
    /// [`search_with_embedding`](Self::search_with_embedding) instead to avoid
    /// holding the lock during the expensive embed step.
    pub async fn search(
        &self,
        query: &str,
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        let query_embedding = self.provider.embed(query).await?;
        self.search_with_embedding(&query_embedding, top_k, min_score)
    }

    /// Search the index with a pre-computed query embedding (fast, in-memory only).
    ///
    /// Use this when the caller embeds the query outside any lock to avoid
    /// serializing concurrent searches across the slow embed I/O step.
    pub fn search_with_embedding(
        &self,
        query_embedding: &[f32],
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        if top_k == 0 || self.entries_by_id.is_empty() {
            return Ok(Vec::new());
        }

        let score_floor = min_score.clamp(0.0, 1.0);
        let fetch_k = compute_fetch_k(self.index.count(), self.entries_by_id.len() as u32, top_k);
        let raw = self
            .index
            .search(query_embedding, fetch_k)
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
}
