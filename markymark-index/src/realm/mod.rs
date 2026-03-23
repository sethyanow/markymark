//! Realm index: multi-document index aggregating document instances.
//!
//! Hybrid arena model: per-document arenas (DocumentIndex owns its Bump),
//! cross-doc lookups use owned String copies that survive document removal.
//! Supports both markdown (DocumentIndex) and structured (StructuredDocumentIndex) documents.

mod cross_doc;
mod helpers;
mod journal;
mod search;
mod types;
pub use types::{
    AnyDocumentIndex, BlockTextMatch, ResolvedBlock, ResolvedCodeSpan, ResolvedHeading,
};

use helpers::detect_journal_date;

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
// Re-exported via `use super::*` in realm/tests/ — must stay in scope.
#[allow(unused_imports)]
use markymark_core::prelude::*;
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
}

impl Default for RealmIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
