//! Cross-document index maintenance methods for the realm index.
//!
//! Handles populating, removing, and incrementally patching cross-doc indexes
//! (headings, blocks, tags, code spans, stems, key paths, journal dates).

use std::collections::{BTreeMap, HashMap, HashSet};

use lasso::Spur;

use markymark_core::DocumentUri;

use super::helpers::detect_journal_date;
use super::types::{AnyDocumentIndex, ResolvedBlock, ResolvedCodeSpan, ResolvedHeading};
use super::{intern_stem, DocContribution, RealmIndex};

use crate::document::DocumentIndex;

// ── Helpers ──

/// Retain entries in a `HashMap<K, Vec<V>>` by key; remove the entry if empty afterward.
fn retain_or_remove_hash<K, V>(
    map: &mut HashMap<K, Vec<V>>,
    key: &K,
    mut predicate: impl FnMut(&V) -> bool,
) where
    K: Eq + std::hash::Hash,
{
    if let Some(entries) = map.get_mut(key) {
        entries.retain(|v| predicate(v));
        if entries.is_empty() {
            map.remove(key);
        }
    }
}

/// Retain entries in a `BTreeMap<K, Vec<V>>` by key; remove the entry if empty afterward.
fn retain_or_remove_btree<K, V>(
    map: &mut BTreeMap<K, Vec<V>>,
    key: &K,
    mut predicate: impl FnMut(&V) -> bool,
) where
    K: Ord,
{
    if let Some(entries) = map.get_mut(key) {
        entries.retain(|v| predicate(v));
        if entries.is_empty() {
            map.remove(key);
        }
    }
}

// ── Cross-doc index methods ──

impl RealmIndex {
    /// Rebuild `tag_to_docs` from contributions if dirty.
    ///
    /// Called from `&mut self` methods before they read/write `tag_to_docs`.
    /// `&self` methods use `tag_counts_from_contributions()` instead.
    pub(super) fn ensure_tags_clean(&mut self) {
        if !self.tags_dirty {
            return;
        }
        self.tag_to_docs.clear();
        for (key, contrib) in &self.contributions {
            if let Some((uri, _)) = self.docs.get(key) {
                for &spur in &contrib.tag_names {
                    self.tag_to_docs.entry(spur).or_default().push(uri.clone());
                }
            }
        }
        self.tags_dirty = false;
    }

    /// Remove a document's entries from cross-doc indexes by URI key.
    pub(super) fn remove_from_cross_doc_indexes(&mut self, key: &str) {
        // Ensure tag index is clean before removal (lazy tag rebuild).
        self.ensure_tags_clean();

        let Some((uri, index)) = self.docs.get(key) else {
            return;
        };

        // Remove from stem index (applies to both markdown and structured docs).
        if let Some(path) = uri.to_file_path() {
            if let Some(stem_str) = path.file_stem().and_then(|s| s.to_str()) {
                let lowered = stem_str.to_ascii_lowercase();
                if let Some(spur) = self.interner.get(&lowered) {
                    retain_or_remove_hash(&mut self.stem_to_uris, &spur, |u| u.as_str() != key);
                }
            }
        }

        match index {
            AnyDocumentIndex::Markdown(md_idx) => {
                // Zero-allocation remove: Spur lookup is O(1), no String collection needed.
                for entry in md_idx.headings() {
                    if let Some(spur) = self.interner.get(entry.slug) {
                        retain_or_remove_hash(&mut self.slug_to_headings, &spur, |(u, _)| {
                            u.as_str() != key
                        });
                    }
                }

                for id in md_idx.block_ids() {
                    if let Some(spur) = self.interner.get(id) {
                        retain_or_remove_hash(&mut self.block_to_location, &spur, |(u, _)| {
                            u.as_str() != key
                        });
                    }
                }

                let mut seen_tags = HashSet::new();
                for tag in md_idx.tags() {
                    if seen_tags.insert(tag.name) {
                        if let Some(spur) = self.interner.get(tag.name) {
                            retain_or_remove_hash(&mut self.tag_to_docs, &spur, |u| {
                                u.as_str() != key
                            });
                        }
                    }
                }

                let mut seen_cs = HashSet::new();
                for cs in md_idx.code_spans() {
                    if seen_cs.insert(cs.text) {
                        if let Some(spur) = self.interner.get(cs.text) {
                            retain_or_remove_hash(&mut self.code_span_to_docs, &spur, |(u, _)| {
                                u.as_str() != key
                            });
                        }
                    }
                }
            }
            AnyDocumentIndex::Structured(st_idx) => {
                let root_paths: Vec<String> =
                    st_idx.root_keys().iter().map(|k| k.path.clone()).collect();
                for path in &root_paths {
                    retain_or_remove_hash(&mut self.key_path_to_docs, path, |u| u.as_str() != key);
                }
            }
        }

        // Clean up journal date index if this was a journal page.
        if let Some(date) = self.uri_to_date.remove(key) {
            retain_or_remove_btree(&mut self.date_to_docs, &date, |u| u.as_str() != key);
        }
    }

    /// Populate all cross-doc indexes for a markdown document (full add).
    /// Used by both add_document and update_document's first-add fallback.
    pub(super) fn populate_cross_doc_indexes(&mut self, uri: &DocumentUri, index: &DocumentIndex) {
        // Headings (Spur-keyed)
        for entry in index.headings() {
            let slug_spur = self.interner.get_or_intern(entry.slug);
            let resolved = ResolvedHeading {
                text: entry.text.to_string(),
                slug: entry.slug.to_string(),
                level: entry.level,
                range: entry.range,
            };
            self.slug_to_headings
                .entry(slug_spur)
                .or_default()
                .push((uri.clone(), resolved));
        }

        // Blocks (Spur-keyed)
        for id in index.block_ids() {
            if let Some(block) = index.block_by_id(id) {
                let id_spur = self.interner.get_or_intern(id);
                self.block_to_location.entry(id_spur).or_default().push((
                    uri.clone(),
                    ResolvedBlock {
                        id: id.to_string(),
                        range: block.range,
                    },
                ));
            }
        }

        // Tags (Spur-keyed, dedup per document)
        let mut seen_tags = HashMap::new();
        for tag in index.tags() {
            if seen_tags.insert(tag.name, ()).is_none() {
                let tag_spur = self.interner.get_or_intern(tag.name);
                self.tag_to_docs
                    .entry(tag_spur)
                    .or_default()
                    .push(uri.clone());
            }
        }

        // Code spans (Spur-keyed, dedup by text per document)
        let mut seen_code_spans = HashMap::new();
        for cs in index.code_spans() {
            if seen_code_spans.insert(cs.text, ()).is_none() {
                let text_spur = self.interner.get_or_intern(cs.text);
                self.code_span_to_docs.entry(text_spur).or_default().push((
                    uri.clone(),
                    ResolvedCodeSpan {
                        text: cs.text.to_string(),
                        range: cs.range,
                        start_byte: cs.start_byte,
                        end_byte: cs.end_byte,
                    },
                ));
            }
        }

        // Stem index for wiki link resolution (Spur-keyed, case-insensitive)
        if let Some(stem_spur) = intern_stem(&mut self.interner, uri) {
            self.stem_to_uris
                .entry(stem_spur)
                .or_default()
                .push(uri.clone());
        }

        // Journal date index
        if let Some(date) = detect_journal_date(uri.as_str()) {
            self.date_to_docs.entry(date).or_default().push(uri.clone());
            self.uri_to_date.insert(uri.as_str().to_string(), date);
        }
    }

    // ── Patch helpers for incremental update_document (Layer 3) ──

    pub(super) fn patch_headings(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        // Remove entries for deleted slugs
        for &spur in old.heading_slugs.difference(&new.heading_slugs) {
            retain_or_remove_hash(&mut self.slug_to_headings, &spur, |(u, _)| {
                u.as_str() != key
            });
        }

        // Build slug → heading entries lookup map for O(1) access per new slug.
        // Without this, each new slug would scan all headings: O(N * H) → O(H²).
        let added: HashSet<&Spur> = new.heading_slugs.difference(&old.heading_slugs).collect();
        if !added.is_empty() {
            let mut slug_map: HashMap<&str, Vec<_>> = HashMap::new();
            for entry in new_index.headings() {
                slug_map.entry(entry.slug).or_default().push(entry);
            }
            for &spur in &added {
                let slug_str = self.interner.resolve(spur);
                if let Some(entries) = slug_map.get(slug_str) {
                    for entry in entries {
                        let resolved = ResolvedHeading {
                            text: entry.text.to_string(),
                            slug: entry.slug.to_string(),
                            level: entry.level,
                            range: entry.range,
                        };
                        self.slug_to_headings
                            .entry(*spur)
                            .or_default()
                            .push((uri.clone(), resolved));
                    }
                }
            }
        }
    }

    pub(super) fn patch_blocks(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        for &spur in old.block_ids.difference(&new.block_ids) {
            retain_or_remove_hash(&mut self.block_to_location, &spur, |(u, _)| {
                u.as_str() != key
            });
        }

        for &spur in new.block_ids.difference(&old.block_ids) {
            let id_str = self.interner.resolve(&spur);
            if let Some(block) = new_index.block_by_id(id_str) {
                self.block_to_location.entry(spur).or_default().push((
                    uri.clone(),
                    ResolvedBlock {
                        id: id_str.to_string(),
                        range: block.range,
                    },
                ));
            }
        }
    }

    pub(super) fn patch_code_spans(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        for &spur in old.code_span_texts.difference(&new.code_span_texts) {
            retain_or_remove_hash(&mut self.code_span_to_docs, &spur, |(u, _)| {
                u.as_str() != key
            });
        }

        for &spur in new.code_span_texts.difference(&old.code_span_texts) {
            let text_str = self.interner.resolve(&spur);
            for cs in new_index.code_spans() {
                if cs.text == text_str {
                    self.code_span_to_docs.entry(spur).or_default().push((
                        uri.clone(),
                        ResolvedCodeSpan {
                            text: cs.text.to_string(),
                            range: cs.range,
                            start_byte: cs.start_byte,
                            end_byte: cs.end_byte,
                        },
                    ));
                    break; // dedup: one entry per unique text per doc
                }
            }
        }
    }

    pub(super) fn patch_stem(
        &mut self,
        old: &DocContribution,
        new: &DocContribution,
        uri: &DocumentUri,
    ) {
        if old.stem == new.stem {
            return;
        }
        let key = uri.as_str();
        // Remove old stem entry
        if let Some(old_spur) = old.stem {
            retain_or_remove_hash(&mut self.stem_to_uris, &old_spur, |u| u.as_str() != key);
        }
        // Add new stem entry
        if let Some(new_spur) = new.stem {
            self.stem_to_uris
                .entry(new_spur)
                .or_default()
                .push(uri.clone());
        }
    }

    pub(super) fn patch_journal_date(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
    ) {
        if old.journal_date == new.journal_date {
            return;
        }
        // Remove old date entry
        if let Some(old_date) = old.journal_date {
            self.uri_to_date.remove(key);
            retain_or_remove_btree(&mut self.date_to_docs, &old_date, |u| u.as_str() != key);
        }
        // Add new date entry
        if let Some(new_date) = new.journal_date {
            self.date_to_docs
                .entry(new_date)
                .or_default()
                .push(uri.clone());
            self.uri_to_date.insert(key.to_string(), new_date);
        }
    }
}
