//! Realm index: multi-document index aggregating document instances.
//!
//! Hybrid arena model: per-document arenas (DocumentIndex owns its Bump),
//! cross-doc lookups use owned String copies that survive document removal.
//! Supports both markdown (DocumentIndex) and structured (StructuredDocumentIndex) documents.

mod helpers;
mod types;
pub use types::{AnyDocumentIndex, ResolvedBlock, ResolvedCodeSpan, ResolvedHeading};

use helpers::{detect_journal_date, resolve_relative_path};

use std::collections::{BTreeMap, HashMap};
#[cfg(feature = "embeddings")]
use std::sync::Arc;

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
    tag_to_docs: HashMap<Spur, Vec<DocumentUri>>,
    /// Code span text → (uri, code span) for cross-doc code span lookups.
    code_span_to_docs: HashMap<Spur, Vec<(DocumentUri, ResolvedCodeSpan)>>,
    /// Key path → URIs of structured docs containing it.
    key_path_to_docs: HashMap<String, Vec<DocumentUri>>,
    /// Journal date → list of URIs for that date (BTreeMap enables range queries by month).
    date_to_docs: BTreeMap<(u16, u8, u8), Vec<DocumentUri>>,
    /// URI → detected journal date for cleanup on removal.
    uri_to_date: HashMap<String, (u16, u8, u8)>,
    /// Optional semantic index for embedding-based search.
    #[cfg(feature = "embeddings")]
    semantic_index: Option<SemanticIndex>,
}

impl RealmIndex {
    /// Create an empty realm index.
    pub fn new() -> Self {
        Self {
            interner: Rodeo::default(),
            docs: HashMap::new(),
            slug_to_headings: HashMap::new(),
            block_to_location: HashMap::new(),
            tag_to_docs: HashMap::new(),
            code_span_to_docs: HashMap::new(),
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
        realm.semantic_index = Some(SemanticIndex::new(provider)?);
        Ok(realm)
    }

    /// Add a markdown document to the realm index.
    /// Populates cross-doc indexes with owned copies.
    pub fn add_document(&mut self, uri: DocumentUri, index: DocumentIndex) {
        let key = uri.as_str().to_string();

        // If replacing, clear old doc from cross-doc indexes first
        self.remove_from_cross_doc_indexes(&key);

        // Populate cross-doc heading index (Spur-keyed, zero allocation for HashMap key)
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

        // Populate cross-doc block index (Spur-keyed)
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

        // Populate cross-doc tag index (Spur-keyed)
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

        // Populate cross-doc code span index (Spur-keyed, dedup by text per document).
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

        #[cfg(feature = "embeddings")]
        if let Some(semantic) = &mut self.semantic_index {
            if let Err(err) = semantic.add_document(uri.clone(), &index) {
                eprintln!(
                    "warning: semantic indexing failed for {}: {err}",
                    uri.as_str()
                );
            }
        }

        // Detect and index journal pages by date pattern in URI filename.
        if let Some(date) = detect_journal_date(uri.as_str()) {
            self.date_to_docs.entry(date).or_default().push(uri.clone());
            self.uri_to_date.insert(uri.as_str().to_string(), date);
        }

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

        self.docs
            .insert(key, (uri, AnyDocumentIndex::Structured(index)));
    }

    /// Remove a document from the realm index.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        let key = uri.as_str().to_string();
        #[cfg(feature = "embeddings")]
        if let Some(semantic) = &mut self.semantic_index {
            semantic.remove_document(uri);
        }
        self.remove_from_cross_doc_indexes(&key);
        self.docs.remove(&key);
    }

    /// Remove a document's entries from cross-doc indexes by URI key.
    fn remove_from_cross_doc_indexes(&mut self, key: &str) {
        let Some((_uri, index)) = self.docs.get(key) else {
            return;
        };

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
    pub fn tag_counts(&self) -> Vec<(String, usize)> {
        self.tag_to_docs
            .iter()
            .map(|(spur, uris)| (self.interner.resolve(spur).to_string(), uris.len()))
            .collect()
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
    pub(crate) fn find_uri_by_stem(&self, target: &str) -> Option<DocumentUri> {
        for (uri, _index) in self.docs.values() {
            if let Some(path) = uri.to_file_path() {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem.eq_ignore_ascii_case(target) {
                        return Some(uri.clone());
                    }
                }
            }
        }
        None
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

    /// Run semantic search if embeddings are enabled.
    ///
    /// Returns an empty vector when semantic indexing is not configured.
    #[cfg(feature = "embeddings")]
    pub fn semantic_search(
        &self,
        query: &str,
        top_k: u32,
        min_score: f32,
    ) -> Result<Vec<SearchResult>, EmbedError> {
        match &self.semantic_index {
            Some(index) => index.search(query, top_k, min_score),
            None => Ok(Vec::new()),
        }
    }

    /// Detect near-duplicate documents if embeddings are enabled.
    ///
    /// Returns an empty vector when semantic indexing is not configured.
    #[cfg(feature = "embeddings")]
    pub fn detect_semantic_duplicates(&self, threshold: f32) -> Vec<DuplicateMatch> {
        match &self.semantic_index {
            Some(index) => index.detect_duplicates(threshold),
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
