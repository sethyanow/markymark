//! Search and lookup methods for the realm index.

use std::collections::HashMap;

use lasso::Spur;

use markymark_core::prelude::*;
use markymark_core::structured::ValueKind;
use markymark_core::DocumentUri;

use super::helpers::resolve_relative_path;
use super::types::{BlockTextMatch, ResolvedBlock, ResolvedCodeSpan, ResolvedHeading};
use super::RealmIndex;

impl RealmIndex {
    /// Look up a heading by slug across all markdown documents.
    pub fn lookup_heading(&self, slug: &str) -> Vec<(DocumentUri, ResolvedHeading)> {
        self.interner
            .get(slug)
            .and_then(|spur| self.slug_to_headings.get(&spur))
            .cloned()
            .unwrap_or_default()
    }

    /// Look up a block by ID across all documents.
    pub fn lookup_block(&self, id: &str) -> Option<(DocumentUri, ResolvedBlock)> {
        self.interner
            .get(id)
            .and_then(|spur| self.block_to_location.get(&spur))
            .and_then(|entries| entries.first().cloned())
    }

    /// Look up documents containing a code span by text across all markdown documents.
    pub fn lookup_code_span(&self, text: &str) -> Vec<(DocumentUri, ResolvedCodeSpan)> {
        self.interner
            .get(text)
            .and_then(|spur| self.code_span_to_docs.get(&spur))
            .cloned()
            .unwrap_or_default()
    }

    /// Get tag usage counts across all markdown documents.
    ///
    /// When `tags_dirty`, computes directly from contributions (read-only,
    /// no mutation needed) so this method stays `&self`.
    pub fn tag_counts(&self) -> Vec<(String, usize)> {
        if self.tags_dirty {
            // Compute from contributions without mutating tag_to_docs.
            let mut counts: HashMap<Spur, usize> = HashMap::new();
            for contrib in self.contributions.values() {
                for &spur in &contrib.tag_names {
                    *counts.entry(spur).or_insert(0) += 1;
                }
            }
            counts
                .into_iter()
                .map(|(spur, count)| (self.interner.resolve(&spur).to_string(), count))
                .collect()
        } else {
            self.tag_to_docs
                .iter()
                .map(|(spur, uris)| (self.interner.resolve(spur).to_string(), uris.len()))
                .collect()
        }
    }

    /// Find a document URI by matching its file stem against a target name.
    /// O(1) via stem_to_uris index. Returns first-added URI when multiple docs share a stem.
    pub(crate) fn find_uri_by_stem(&self, target: &str) -> Option<DocumentUri> {
        let lowered = target.to_ascii_lowercase();
        self.interner
            .get(&lowered)
            .and_then(|spur| self.stem_to_uris.get(&spur))
            .and_then(|uris| uris.first().cloned())
    }

    /// Find a document URI by resolving `relative_url` relative to `from_uri`'s directory.
    ///
    /// Returns `None` if the resolved path is not present in the realm.
    pub(crate) fn find_uri_by_relative_path(
        &self,
        from_uri: &DocumentUri,
        relative_url: &str,
    ) -> Option<DocumentUri> {
        let from_path = from_uri.to_file_path()?;
        let parent = from_path.parent()?;
        // Resolve the relative URL against the parent directory, then canonicalise components.
        let resolved = resolve_relative_path(parent, relative_url);
        let candidate = DocumentUri::from_file_path(&resolved);
        // Check whether the resolved URI is present in the realm.
        if self.docs.contains_key(candidate.as_str()) {
            Some(candidate)
        } else {
            None
        }
    }

    /// Search key paths across all structured documents.
    /// Returns (uri, path, key, value_kind, range) tuples.
    pub fn search_key_paths(
        &self,
        query: &str,
    ) -> Vec<(DocumentUri, String, String, ValueKind, Range)> {
        let mut results = Vec::new();
        for (uri, idx) in self.iter_structured_documents() {
            for entry in idx.search_keys(query) {
                results.push((
                    uri.clone(),
                    entry.path.clone(),
                    entry.key.clone(),
                    entry.value_kind,
                    entry.key_range,
                ));
            }
        }
        results
    }

    /// Search block text across all markdown documents (case-insensitive substring).
    ///
    /// Returns up to `limit` matches. The second element of the tuple is `true` when
    /// the total number of matches exceeded `limit` (i.e. results were truncated).
    ///
    /// `kind_filter` restricts matches to a specific `BlockKind`.
    /// `include_text` controls whether the block text is included in results.
    pub fn search_block_text(
        &self,
        query: &str,
        kind_filter: Option<crate::document::BlockKind>,
        limit: usize,
        include_text: bool,
    ) -> (Vec<BlockTextMatch>, bool) {
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        let mut total_found: usize = 0;

        for (uri, doc) in self.iter_documents() {
            let headings = doc.headings();
            let content_blocks = doc.content_blocks();

            for block in content_blocks {
                if let Some(ref kind) = kind_filter {
                    if &block.kind != kind {
                        continue;
                    }
                }

                let text = doc.block_text(block);
                if text.is_empty() {
                    continue;
                }
                if !text.to_lowercase().contains(&query_lower) {
                    continue;
                }

                total_found += 1;

                if matches.len() < limit {
                    let parent_slug = block
                        .parent_heading
                        .and_then(|idx| headings.get(idx).map(|h| h.slug.to_string()));

                    matches.push(BlockTextMatch {
                        uri: uri.clone(),
                        kind: block.kind,
                        range: block.range,
                        parent_heading_slug: parent_slug,
                        block_id: block.block_id.map(|s| s.to_string()),
                        text: if include_text {
                            Some(text.to_string())
                        } else {
                            None
                        },
                    });
                }
            }
        }

        (matches, total_found > limit)
    }
}
