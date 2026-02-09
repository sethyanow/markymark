//! Realm index: multi-document index aggregating DocumentIndex instances.

use std::collections::HashMap;

use crate::document::{BlockEntry, DocumentIndex, HeadingEntry};
use markymark_core::DocumentUri;

/// A multi-document index that aggregates [`DocumentIndex`] instances
/// and provides global cross-document lookups.
pub struct RealmIndex {
    docs: HashMap<String, (DocumentUri, DocumentIndex)>,
}

impl RealmIndex {
    /// Create an empty realm index.
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    /// Add a document to the realm index.
    pub fn add_document(&mut self, uri: DocumentUri, index: DocumentIndex) {
        let key = uri.as_str().to_string();
        self.docs.insert(key, (uri, index));
    }

    /// Remove a document from the realm index.
    pub fn remove_document(&mut self, uri: &DocumentUri) {
        self.docs.remove(uri.as_str());
    }

    /// Number of documents in the realm.
    pub fn document_count(&self) -> usize {
        self.docs.len()
    }

    /// Look up a heading by slug across all documents.
    pub fn lookup_heading(&self, slug: &str) -> Vec<(&DocumentUri, &HeadingEntry)> {
        let mut results = Vec::new();
        for (uri, index) in self.docs.values() {
            if let Some(entry) = index.heading_by_slug(slug) {
                results.push((uri, entry));
            }
        }
        results
    }

    /// Look up a block by ID across all documents.
    pub fn lookup_block(&self, id: &str) -> Option<(&DocumentUri, &BlockEntry)> {
        for (uri, index) in self.docs.values() {
            if let Some(entry) = index.block_by_id(id) {
                return Some((uri, entry));
            }
        }
        None
    }

    /// Get tag usage counts across all documents.
    ///
    /// Returns a vec of (tag_name, document_count) pairs where
    /// the count is the number of documents containing that tag.
    pub fn tag_counts(&self) -> Vec<(&str, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (_uri, index) in self.docs.values() {
            // Collect unique tag names within this single document
            let mut seen_in_doc: HashMap<&str, bool> = HashMap::new();
            for tag in index.tags() {
                seen_in_doc.entry(tag.name.as_str()).or_insert(true);
            }
            for tag_name in seen_in_doc.keys() {
                *counts.entry(tag_name).or_insert(0) += 1;
            }
        }
        counts.into_iter().collect()
    }

    /// Get a specific document's index by URI.
    pub fn get_document(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.docs.get(uri.as_str()).map(|(_uri, index)| index)
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
