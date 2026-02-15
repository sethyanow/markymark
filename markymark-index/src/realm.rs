//! Realm index: multi-document index aggregating DocumentIndex instances.
//!
//! Hybrid arena model: per-document arenas (DocumentIndex owns its Bump),
//! cross-doc lookups use owned String copies that survive document removal.

use std::collections::HashMap;

use crate::document::DocumentIndex;
use markymark_core::prelude::*;
use markymark_core::DocumentUri;

/// Owned copy of heading data for cross-document lookups.
/// Survives removal of individual documents (unlike arena-backed document entries).
#[derive(Debug, Clone)]
pub struct ResolvedHeading {
    /// The heading text.
    pub text: String,
    /// URL-safe slug.
    pub slug: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range.
    pub range: Range,
}

/// Owned copy of block data for cross-document lookups.
#[derive(Debug, Clone)]
pub struct ResolvedBlock {
    /// Block identifier.
    pub id: String,
    /// Source range.
    pub range: Range,
}

/// A multi-document index that aggregates [`DocumentIndex`] instances
/// and provides global cross-document lookups using owned storage.
pub struct RealmIndex {
    docs: HashMap<String, (DocumentUri, DocumentIndex)>,
    /// Slug → (uri, owned heading). Owned copies survive doc removal.
    slug_to_headings: HashMap<String, Vec<(DocumentUri, ResolvedHeading)>>,
    /// Block id → list of (uri, block) in insertion order.
    /// Multiple docs may contain the same block id.
    block_to_location: HashMap<String, Vec<(DocumentUri, ResolvedBlock)>>,
    /// Tag name → URIs of docs containing it. For tag_counts.
    tag_to_docs: HashMap<String, Vec<DocumentUri>>,
}

impl RealmIndex {
    /// Create an empty realm index.
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
            slug_to_headings: HashMap::new(),
            block_to_location: HashMap::new(),
            tag_to_docs: HashMap::new(),
        }
    }

    /// Add a document to the realm index.
    /// Populates cross-doc indexes with owned copies so lookups survive doc removal.
    /// If replacing an existing doc with the same URI, clears old entries first.
    pub fn add_document(&mut self, uri: DocumentUri, index: DocumentIndex) {
        let key = uri.as_str().to_string();

        // If replacing, clear old doc from cross-doc indexes first
        self.remove_from_cross_doc_indexes(&key);

        // Populate cross-doc heading index (owned copies)
        for entry in index.headings() {
            let resolved = ResolvedHeading {
                text: entry.text.to_string(),
                slug: entry.slug.to_string(),
                level: entry.level,
                range: entry.range,
            };
            self.slug_to_headings
                .entry(entry.slug.to_string())
                .or_default()
                .push((uri.clone(), resolved));
        }

        // Populate cross-doc block index (owned copies)
        for id in index.block_ids() {
            if let Some(block) = index.block_by_id(id) {
                self.block_to_location
                    .entry(id.to_string())
                    .or_default()
                    .push((
                        uri.clone(),
                        ResolvedBlock {
                            id: id.to_string(),
                            range: block.range,
                        },
                    ));
            }
        }

        // Populate cross-doc tag index (owned copies)
        let mut seen_tags = HashMap::new();
        for tag in index.tags() {
            if seen_tags.insert(tag.name, ()).is_none() {
                self.tag_to_docs
                    .entry(tag.name.to_string())
                    .or_default()
                    .push(uri.clone());
            }
        }

        self.docs.insert(key, (uri, index));
    }

    /// Remove a document from the realm index.
    /// Cleans cross-doc indexes so no stale refs remain.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        let key = uri.as_str().to_string();
        self.remove_from_cross_doc_indexes(&key);
        self.docs.remove(&key);
    }

    /// Remove a document's entries from cross-doc indexes by URI key.
    fn remove_from_cross_doc_indexes(&mut self, key: &str) {
        self.slug_to_headings
            .values_mut()
            .for_each(|v| v.retain(|(u, _)| u.as_str() != key));
        self.slug_to_headings.retain(|_, v| !v.is_empty());

        self.block_to_location
            .values_mut()
            .for_each(|v| v.retain(|(u, _)| u.as_str() != key));
        self.block_to_location.retain(|_, v| !v.is_empty());

        self.tag_to_docs
            .values_mut()
            .for_each(|v| v.retain(|u| u.as_str() != key));
        self.tag_to_docs.retain(|_, v| !v.is_empty());
    }

    /// Number of documents in the realm.
    pub fn document_count(&self) -> usize {
        self.docs.len()
    }

    /// Look up a heading by slug across all documents.
    /// Returns owned [`ResolvedHeading`] copies from the cross-doc index.
    pub fn lookup_heading(&self, slug: &str) -> Vec<(DocumentUri, ResolvedHeading)> {
        self.slug_to_headings.get(slug).cloned().unwrap_or_default()
    }

    /// Look up a block by ID across all documents.
    /// Returns the first-inserted [`ResolvedBlock`] from the cross-doc index.
    /// Multiple documents may define the same block ID; this returns the first match.
    pub fn lookup_block(&self, id: &str) -> Option<(DocumentUri, ResolvedBlock)> {
        self.block_to_location
            .get(id)
            .and_then(|entries| entries.first().cloned())
    }

    /// Get tag usage counts across all documents.
    ///
    /// Returns a vec of (tag_name, document_count) pairs where
    /// the count is the number of documents containing that tag.
    pub fn tag_counts(&self) -> Vec<(String, usize)> {
        self.tag_to_docs
            .iter()
            .map(|(name, uris)| (name.clone(), uris.len()))
            .collect()
    }

    /// Get a specific document's index by URI.
    pub fn get_document(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.docs.get(uri.as_str()).map(|(_uri, index)| index)
    }

    /// Iterate over all documents in the realm as `(uri, index)` pairs.
    pub fn iter_documents(&self) -> impl Iterator<Item = (&DocumentUri, &DocumentIndex)> {
        self.docs.values().map(|(uri, index)| (uri, index))
    }

    /// Find a document URI by matching its file stem against a target name
    /// (case-insensitive). Used by the resolution module.
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
}

impl Default for RealmIndex {
    fn default() -> Self {
        Self::new()
    }
}
