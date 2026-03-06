//! Realm index: multi-document index aggregating document instances.
//!
//! Hybrid arena model: per-document arenas (DocumentIndex owns its Bump),
//! cross-doc lookups use owned String copies that survive document removal.
//! Supports both markdown (DocumentIndex) and structured (StructuredDocumentIndex) documents.

mod helpers;
mod types;
pub use types::{AnyDocumentIndex, BlockTextMatch, ResolvedBlock, ResolvedCodeSpan, ResolvedHeading};

use helpers::{detect_journal_date, resolve_relative_path};

use std::collections::{BTreeMap, HashMap, HashSet};
#[cfg(feature = "embeddings")]
use std::sync::Arc;
#[cfg(feature = "embeddings")]
use tokio::sync::Mutex as TokioMutex;

use lasso::{Rodeo, Spur};

use crate::document::DocumentIndex;
#[cfg(feature = "embeddings")]
use crate::semantic::{DuplicateMatch, SearchResult, SemanticIndex};
use crate::structured_document::StructuredDocumentIndex;
use markymark_core::prelude::*;
use markymark_core::structured::ValueKind;
use markymark_core::DocumentUri;

/// A multi-document index that aggregates document instances
/// and provides global cross-document lookups using owned storage.
pub struct RealmIndex {
    /// String interner for cross-doc HashMap keys (slugs, tags, block IDs).
    /// Grows monotonically; never deallocates. For a 10K-doc vault with ~500K
    /// unique slugs/tags/blocks, interner holds ~10MB. Acceptable for LSP lifetime.
    interner: Rodeo,
    docs: HashMap<String, (DocumentUri, AnyDocumentIndex)>,
    /// Slug → (uri, owned heading). Owned copies survive doc removal.
    slug_to_headings: HashMap<Spur, Vec<(DocumentUri, ResolvedHeading)>>,
    /// Block id → list of (uri, block) in insertion order.
    block_to_location: HashMap<Spur, Vec<(DocumentUri, ResolvedBlock)>>,
    /// Tag name → URIs of docs containing it.
    /// Lazily maintained: set `tags_dirty = true` during `update_document`
    /// instead of patching eagerly. Rebuilt from `contributions` on next
    /// mutation that needs it, or computed on-the-fly in read-only queries.
    tag_to_docs: HashMap<Spur, Vec<DocumentUri>>,
    /// When true, `tag_to_docs` does not reflect recent `update_document` changes.
    /// `&mut self` methods call `ensure_tags_clean()` before reading tag_to_docs.
    /// `&self` methods compute directly from contributions when dirty.
    tags_dirty: bool,
    /// Code span text → (uri, code span) for cross-doc code span lookups.
    code_span_to_docs: HashMap<Spur, Vec<(DocumentUri, ResolvedCodeSpan)>>,
    /// Per-document contribution metadata for incremental cross-doc index updates.
    contributions: HashMap<String, DocContribution>,
    /// File stem → URIs. Stems are lowercased before interning for case-insensitive lookup.
    /// Multiple URIs can share a stem (e.g., /a/readme.md and /b/readme.md).
    /// find_uri_by_stem returns the first entry (insertion order).
    stem_to_uris: HashMap<Spur, Vec<DocumentUri>>,
    /// Key path → URIs of structured docs containing it.
    key_path_to_docs: HashMap<String, Vec<DocumentUri>>,
    /// Journal date → list of URIs for that date (BTreeMap enables range queries by month).
    date_to_docs: BTreeMap<(u16, u8, u8), Vec<DocumentUri>>,
    /// URI → detected journal date for cleanup on removal.
    uri_to_date: HashMap<String, (u16, u8, u8)>,
    /// Optional semantic index for embedding-based search.
    ///
    /// Wrapped in `Arc<TokioMutex>` so callers (e.g. MCP engine) can clone
    /// the handle, release outer realm locks, and then await search without
    /// blocking realm-level writes.
    #[cfg(feature = "embeddings")]
    semantic_index: Option<Arc<TokioMutex<SemanticIndex>>>,
}

/// Extract the file stem from a DocumentUri, lowercase it, and intern via Rodeo.
/// Returns None for URIs without a valid file path or stem (e.g., untitled: URIs).
fn intern_stem(interner: &mut Rodeo, uri: &DocumentUri) -> Option<Spur> {
    let path = uri.to_file_path()?;
    let stem = path.file_stem()?.to_str()?;
    let lowered = stem.to_ascii_lowercase();
    Some(interner.get_or_intern(&lowered))
}

/// Tracks what Spur keys a document contributed to each cross-doc index.
/// Used to diff old vs new contributions on update, patching only changed entries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DocContribution {
    heading_slugs: HashSet<Spur>,
    block_ids: HashSet<Spur>,
    tag_names: HashSet<Spur>,
    code_span_texts: HashSet<Spur>,
    stem: Option<Spur>,
    journal_date: Option<(u16, u8, u8)>,
}

impl DocContribution {
    fn build(interner: &mut Rodeo, index: &DocumentIndex, uri: &DocumentUri) -> Self {
        let mut heading_slugs = HashSet::new();
        for entry in index.headings() {
            heading_slugs.insert(interner.get_or_intern(entry.slug));
        }

        let mut block_ids = HashSet::new();
        for id in index.block_ids() {
            block_ids.insert(interner.get_or_intern(id));
        }

        let mut tag_names = HashSet::new();
        for tag in index.tags() {
            tag_names.insert(interner.get_or_intern(tag.name));
        }

        let mut code_span_texts = HashSet::new();
        for cs in index.code_spans() {
            code_span_texts.insert(interner.get_or_intern(cs.text));
        }

        let stem = intern_stem(interner, uri);
        let journal_date = detect_journal_date(uri.as_str());

        Self {
            heading_slugs,
            block_ids,
            tag_names,
            code_span_texts,
            stem,
            journal_date,
        }
    }
}

impl RealmIndex {
    /// Create an empty realm index.
    pub fn new() -> Self {
        Self {
            interner: Rodeo::default(),
            docs: HashMap::new(),
            contributions: HashMap::new(),
            slug_to_headings: HashMap::new(),
            block_to_location: HashMap::new(),
            tag_to_docs: HashMap::new(),
            tags_dirty: false,
            code_span_to_docs: HashMap::new(),
            stem_to_uris: HashMap::new(),
            key_path_to_docs: HashMap::new(),
            date_to_docs: BTreeMap::new(),
            uri_to_date: HashMap::new(),
            #[cfg(feature = "embeddings")]
            semantic_index: None,
        }
    }

    /// Create a realm index with semantic embeddings enabled.
    #[cfg(feature = "embeddings")]
    pub fn new_with_embeddings(provider: Arc<dyn EmbeddingProvider>) -> Result<Self, EmbedError> {
        let mut realm = Self::new();
        realm.semantic_index = Some(Arc::new(TokioMutex::new(SemanticIndex::new(provider)?)));
        Ok(realm)
    }

    /// Add a markdown document to the realm index (structural + semantic embedding).
    ///
    /// For batch operations (e.g. `AddRoot`) where the caller manages lock scope,
    /// use [`add_document_structural`] + deferred embedding via [`semantic_index_arc`]
    /// to avoid holding outer locks during slow embedding I/O.
    pub async fn add_document(&mut self, uri: DocumentUri, index: DocumentIndex) {
        #[cfg(feature = "embeddings")]
        if let Some(semantic) = &self.semantic_index {
            let mut guard = semantic.lock().await;
            if let Err(err) = guard.add_document(uri.clone(), &index).await {
                eprintln!(
                    "warning: semantic indexing failed for {}: {err}",
                    uri.as_str()
                );
            }
        }

        self.add_document_structural(uri, index);
    }

    /// Add multiple markdown documents to the realm index.
    ///
    /// Embeddings (when enabled) are generated in a single semantic batch,
    /// then structural indexes are updated for each document.
    pub async fn add_documents(&mut self, docs: Vec<(DocumentUri, DocumentIndex)>) {
        #[cfg(feature = "embeddings")]
        if let Some(semantic) = &self.semantic_index {
            let semantic_docs = docs
                .iter()
                .map(|(uri, index)| (uri.clone(), index))
                .collect::<Vec<_>>();
            let mut guard = semantic.lock().await;
            if let Err(err) = guard.add_documents(semantic_docs).await {
                eprintln!("warning: semantic indexing failed for document batch: {err}",);
            }
        }

        for (uri, index) in docs {
            self.add_document_structural(uri, index);
        }
    }

    /// Add a markdown document to the structural index only (no semantic embedding).
    ///
    /// This is the sync portion of [`add_document`]. It updates cross-doc indexes,
    /// contribution metadata, and document storage. Embedding is the caller's
    /// responsibility — clone the [`semantic_index_arc`] and embed outside any
    /// outer lock to avoid blocking concurrent operations.
    pub fn add_document_structural(&mut self, uri: DocumentUri, index: DocumentIndex) {
        let key = uri.as_str().to_string();

        // If replacing, clear old doc from cross-doc indexes first
        self.remove_from_cross_doc_indexes(&key);

        self.populate_cross_doc_indexes(&uri, &index);

        // Store contribution metadata for incremental updates (Layer 3).
        let contrib = DocContribution::build(&mut self.interner, &index, &uri);
        self.contributions.insert(key.clone(), contrib);

        self.docs
            .insert(key, (uri, AnyDocumentIndex::Markdown(index)));
    }

    /// Add a structured document to the realm index.
    /// Populates key path cross-doc index for search-symbols.
    pub fn add_structured_document(&mut self, uri: DocumentUri, index: StructuredDocumentIndex) {
        let key = uri.as_str().to_string();

        // If replacing, clear old doc from cross-doc indexes first
        self.remove_from_cross_doc_indexes(&key);

        // Populate cross-doc key path index (root keys only for efficiency)
        for entry in index.root_keys() {
            self.key_path_to_docs
                .entry(entry.path.clone())
                .or_default()
                .push(uri.clone());
        }

        // Populate stem index for structured documents too.
        if let Some(stem_spur) = intern_stem(&mut self.interner, &uri) {
            self.stem_to_uris
                .entry(stem_spur)
                .or_default()
                .push(uri.clone());
        }

        self.docs
            .insert(key, (uri, AnyDocumentIndex::Structured(index)));
    }

    /// Incrementally update a markdown document's cross-doc indexes.
    ///
    /// Diffs old vs new contributions (heading slugs, block IDs, tags, code spans)
    /// and patches only the changed entries. For the common case (single-char edit
    /// that doesn't change structure), this skips all cross-doc index operations.
    ///
    /// Tags are lazily deferred: instead of patching `tag_to_docs` eagerly, we set
    /// `tags_dirty = true`. The tag index is rebuilt from contributions on the next
    /// mutation that needs it, or computed on-the-fly in read-only queries.
    pub async fn update_document(&mut self, uri: DocumentUri, new_index: DocumentIndex) {
        let key = uri.as_str().to_string();
        let new_contrib = DocContribution::build(&mut self.interner, &new_index, &uri);

        // Remove old contribution to get owned access (avoids clone of 4 HashSets).
        let old_contrib = self.contributions.remove(&key);

        if let Some(ref old_contrib) = old_contrib {
            if old_contrib == &new_contrib {
                // Fast path: contribution sets identical — skip cross-doc index ops.
                // Still update semantic index: heading text may have changed even
                // though slugs are identical (e.g. "Foo!" → "Foo").
                #[cfg(feature = "embeddings")]
                if let Some(semantic) = &self.semantic_index {
                    let mut guard = semantic.lock().await;
                    if let Err(err) = guard.update_document(uri.clone(), &new_index).await {
                        log::warn!("semantic indexing failed for {}: {err}", uri.as_str());
                    }
                }
            } else {
                // Slow path: diff and patch only changed entries.
                self.patch_headings(&key, &uri, old_contrib, &new_contrib, &new_index);
                self.patch_blocks(&key, &uri, old_contrib, &new_contrib, &new_index);
                // Tags are lazily deferred — mark dirty instead of patching eagerly.
                if old_contrib.tag_names != new_contrib.tag_names {
                    self.tags_dirty = true;
                }
                self.patch_code_spans(&key, &uri, old_contrib, &new_contrib, &new_index);
                self.patch_stem(old_contrib, &new_contrib, &uri);
                self.patch_journal_date(&key, &uri, old_contrib, &new_contrib);

                // Incrementally update semantic index (only re-embeds changed headings).
                #[cfg(feature = "embeddings")]
                if let Some(semantic) = &self.semantic_index {
                    let mut guard = semantic.lock().await;
                    if let Err(err) = guard.update_document(uri.clone(), &new_index).await {
                        log::warn!("semantic indexing failed for {}: {err}", uri.as_str());
                    }
                }
            }
        } else {
            // First add (no prior contribution): full population.
            self.ensure_tags_clean();
            self.populate_cross_doc_indexes(&uri, &new_index);

            #[cfg(feature = "embeddings")]
            if let Some(semantic) = &self.semantic_index {
                let mut guard = semantic.lock().await;
                if let Err(err) = guard.add_document(uri.clone(), &new_index).await {
                    log::warn!("semantic indexing failed for {}: {err}", uri.as_str());
                }
            }
        }

        self.contributions.insert(key.clone(), new_contrib);
        self.docs
            .insert(key, (uri, AnyDocumentIndex::Markdown(new_index)));
    }

    /// Remove a document from the realm index.
    pub async fn remove_document(&mut self, uri: &DocumentUri) {
        let key = uri.as_str().to_string();
        #[cfg(feature = "embeddings")]
        if let Some(semantic) = &self.semantic_index {
            semantic.lock().await.remove_document(uri);
        }
        self.remove_from_cross_doc_indexes(&key);
        self.contributions.remove(&key);
        self.docs.remove(&key);
    }

    /// Rebuild `tag_to_docs` from contributions if dirty.
    ///
    /// Called from `&mut self` methods before they read/write `tag_to_docs`.
    /// `&self` methods use `tag_counts_from_contributions()` instead.
    fn ensure_tags_clean(&mut self) {
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
    fn remove_from_cross_doc_indexes(&mut self, key: &str) {
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
                    if let Some(uris) = self.stem_to_uris.get_mut(&spur) {
                        uris.retain(|u| u.as_str() != key);
                        if uris.is_empty() {
                            self.stem_to_uris.remove(&spur);
                        }
                    }
                }
            }
        }

        match index {
            AnyDocumentIndex::Markdown(md_idx) => {
                // Zero-allocation remove: Spur lookup is O(1), no String collection needed.
                for entry in md_idx.headings() {
                    if let Some(spur) = self.interner.get(entry.slug) {
                        if let Some(entries) = self.slug_to_headings.get_mut(&spur) {
                            entries.retain(|(u, _)| u.as_str() != key);
                            if entries.is_empty() {
                                self.slug_to_headings.remove(&spur);
                            }
                        }
                    }
                }

                for id in md_idx.block_ids() {
                    if let Some(spur) = self.interner.get(id) {
                        if let Some(entries) = self.block_to_location.get_mut(&spur) {
                            entries.retain(|(u, _)| u.as_str() != key);
                            if entries.is_empty() {
                                self.block_to_location.remove(&spur);
                            }
                        }
                    }
                }

                let mut seen_tags = std::collections::HashSet::new();
                for tag in md_idx.tags() {
                    if seen_tags.insert(tag.name) {
                        if let Some(spur) = self.interner.get(tag.name) {
                            if let Some(uris) = self.tag_to_docs.get_mut(&spur) {
                                uris.retain(|u| u.as_str() != key);
                                if uris.is_empty() {
                                    self.tag_to_docs.remove(&spur);
                                }
                            }
                        }
                    }
                }

                let mut seen_cs = std::collections::HashSet::new();
                for cs in md_idx.code_spans() {
                    if seen_cs.insert(cs.text) {
                        if let Some(spur) = self.interner.get(cs.text) {
                            if let Some(entries) = self.code_span_to_docs.get_mut(&spur) {
                                entries.retain(|(u, _)| u.as_str() != key);
                                if entries.is_empty() {
                                    self.code_span_to_docs.remove(&spur);
                                }
                            }
                        }
                    }
                }
            }
            AnyDocumentIndex::Structured(st_idx) => {
                let root_paths: Vec<String> =
                    st_idx.root_keys().iter().map(|k| k.path.clone()).collect();
                for path in &root_paths {
                    if let Some(uris) = self.key_path_to_docs.get_mut(path) {
                        uris.retain(|u| u.as_str() != key);
                        if uris.is_empty() {
                            self.key_path_to_docs.remove(path);
                        }
                    }
                }
            }
        }

        // Clean up journal date index if this was a journal page.
        if let Some(date) = self.uri_to_date.remove(key) {
            if let Some(uris) = self.date_to_docs.get_mut(&date) {
                uris.retain(|u| u.as_str() != key);
                if uris.is_empty() {
                    self.date_to_docs.remove(&date);
                }
            }
        }
    }

    /// Populate all cross-doc indexes for a markdown document (full add).
    /// Used by both add_document and update_document's first-add fallback.
    fn populate_cross_doc_indexes(&mut self, uri: &DocumentUri, index: &DocumentIndex) {
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

    fn patch_headings(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        // Remove entries for deleted slugs
        for &spur in old.heading_slugs.difference(&new.heading_slugs) {
            if let Some(entries) = self.slug_to_headings.get_mut(&spur) {
                entries.retain(|(u, _)| u.as_str() != key);
                if entries.is_empty() {
                    self.slug_to_headings.remove(&spur);
                }
            }
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

    fn patch_blocks(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        for &spur in old.block_ids.difference(&new.block_ids) {
            if let Some(entries) = self.block_to_location.get_mut(&spur) {
                entries.retain(|(u, _)| u.as_str() != key);
                if entries.is_empty() {
                    self.block_to_location.remove(&spur);
                }
            }
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

    fn patch_code_spans(
        &mut self,
        key: &str,
        uri: &DocumentUri,
        old: &DocContribution,
        new: &DocContribution,
        new_index: &DocumentIndex,
    ) {
        for &spur in old.code_span_texts.difference(&new.code_span_texts) {
            if let Some(entries) = self.code_span_to_docs.get_mut(&spur) {
                entries.retain(|(u, _)| u.as_str() != key);
                if entries.is_empty() {
                    self.code_span_to_docs.remove(&spur);
                }
            }
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

    fn patch_stem(&mut self, old: &DocContribution, new: &DocContribution, uri: &DocumentUri) {
        if old.stem == new.stem {
            return;
        }
        let key = uri.as_str();
        // Remove old stem entry
        if let Some(old_spur) = old.stem {
            if let Some(uris) = self.stem_to_uris.get_mut(&old_spur) {
                uris.retain(|u| u.as_str() != key);
                if uris.is_empty() {
                    self.stem_to_uris.remove(&old_spur);
                }
            }
        }
        // Add new stem entry
        if let Some(new_spur) = new.stem {
            self.stem_to_uris
                .entry(new_spur)
                .or_default()
                .push(uri.clone());
        }
    }

    fn patch_journal_date(
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
            if let Some(uris) = self.date_to_docs.get_mut(&old_date) {
                uris.retain(|u| u.as_str() != key);
                if uris.is_empty() {
                    self.date_to_docs.remove(&old_date);
                }
            }
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

    /// Number of documents in the realm (markdown + structured).
    pub fn document_count(&self) -> usize {
        self.docs.len()
    }

    /// Number of markdown documents in the realm.
    pub fn markdown_count(&self) -> usize {
        self.docs
            .values()
            .filter(|(_, idx)| idx.is_markdown())
            .count()
    }

    /// Number of structured documents in the realm.
    pub fn structured_count(&self) -> usize {
        self.docs
            .values()
            .filter(|(_, idx)| idx.is_structured())
            .count()
    }

    /// Number of unique strings held by the interner (slugs, tags, block IDs, code spans, stems).
    pub fn interner_len(&self) -> usize {
        self.interner.len()
    }

    /// Total number of key paths across all structured documents.
    pub fn key_path_count(&self) -> usize {
        self.docs
            .values()
            .filter_map(|(_, idx)| idx.as_structured())
            .map(|st| st.key_count())
            .sum()
    }

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

    /// Get a markdown document's index by URI.
    /// Returns `None` for structured documents — use [`get_any_document`] instead.
    pub fn get_document(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.docs
            .get(uri.as_str())
            .and_then(|(_, idx)| idx.as_markdown())
    }

    /// Get any document's index (markdown or structured) by URI.
    pub fn get_any_document(&self, uri: &DocumentUri) -> Option<&AnyDocumentIndex> {
        self.docs.get(uri.as_str()).map(|(_, idx)| idx)
    }

    /// Get a structured document's index by URI.
    pub fn get_structured_document(&self, uri: &DocumentUri) -> Option<&StructuredDocumentIndex> {
        self.docs
            .get(uri.as_str())
            .and_then(|(_, idx)| idx.as_structured())
    }

    /// Iterate over all markdown documents in the realm.
    pub fn iter_documents(&self) -> impl Iterator<Item = (&DocumentUri, &DocumentIndex)> {
        self.docs
            .values()
            .filter_map(|(uri, idx)| idx.as_markdown().map(|md| (uri, md)))
    }

    /// Iterate over all documents (markdown and structured) in the realm.
    pub fn iter_all_documents(&self) -> impl Iterator<Item = (&DocumentUri, &AnyDocumentIndex)> {
        self.docs.values().map(|(uri, idx)| (uri, idx))
    }

    /// Iterate over all structured documents in the realm.
    pub fn iter_structured_documents(
        &self,
    ) -> impl Iterator<Item = (&DocumentUri, &StructuredDocumentIndex)> {
        self.docs
            .values()
            .filter_map(|(uri, idx)| idx.as_structured().map(|st| (uri, st)))
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

    /// Get a cloneable handle to the semantic index.
    ///
    /// Callers can clone this `Arc`, release outer realm locks, and then
    /// lock the inner `Mutex` to run searches without blocking realm-level
    /// write operations.
    #[cfg(feature = "embeddings")]
    pub fn semantic_index_arc(&self) -> Option<Arc<TokioMutex<SemanticIndex>>> {
        self.semantic_index.clone()
    }

    /// Run semantic search if embeddings are enabled.
    ///
    /// **Warning**: this method locks the inner `TokioMutex` internally, which
    /// means the caller's borrow on `&self` is held for the duration of the
    /// search. If you need to release an outer realm lock before searching,
    /// use [`semantic_index_arc`] instead and lock the `Arc` yourself.
    #[cfg(feature = "embeddings")]
    pub async fn semantic_search(
        &self,
        query: &str,
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        match &self.semantic_index {
            Some(sem) => {
                let guard = sem.lock().await;
                guard.search(query, top_k, min_score).await
            }
            None => Ok(Vec::new()),
        }
    }

    /// Detect near-duplicate documents if embeddings are enabled.
    ///
    /// Returns an empty vector when semantic indexing is not configured.
    #[cfg(feature = "embeddings")]
    pub async fn detect_semantic_duplicates(&self, threshold: f32) -> Vec<DuplicateMatch> {
        match &self.semantic_index {
            Some(sem) => {
                let guard = sem.lock().await;
                guard.detect_duplicates(threshold)
            }
            None => Vec::new(),
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
                    let parent_slug = block.parent_heading.and_then(|idx| {
                        headings.get(idx).map(|h| h.slug.to_string())
                    });

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

    /// Returns all journal documents for a given year and month, sorted by day ascending.
    /// The tuple is `(DocumentUri, day)` so callers can sort or filter by specific dates.
    pub fn lookup_journal_by_month(&self, year: u16, month: u8) -> Vec<(DocumentUri, u8)> {
        let start = (year, month, 1u8);
        let end = (year, month, 31u8);
        self.date_to_docs
            .range(start..=end)
            .flat_map(|((_, _, d), uris)| uris.iter().map(move |u| (u.clone(), *d)))
            .collect()
    }

    /// Returns the detected journal date `(year, month, day)` for a URI, or `None`
    /// if the URI does not correspond to a journal page.
    pub fn journal_date(&self, uri: &DocumentUri) -> Option<(u16, u8, u8)> {
        self.uri_to_date.get(uri.as_str()).copied()
    }
}

impl Default for RealmIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
