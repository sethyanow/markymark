//! Server state: document store, parsing, and indexing.

use std::collections::HashMap;

use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;

/// The internal state of the LSP server.
///
/// Manages document text storage, parsed ASTs, and the realm index.
#[derive(Default)]
pub struct ServerState {
    /// Raw document text keyed by URI string.
    documents: HashMap<String, String>,
    /// The realm index for cross-document lookups.
    realm: RealmIndex,
}

impl ServerState {
    /// Create a new empty server state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse text and build a document index.
    fn build_index(text: &str) -> DocumentIndex {
        let mut parser = Parser::new().expect("failed to create parser");
        let ast = parser.parse(text).expect("failed to parse document");
        DocumentIndex::from_ast(&ast)
    }

    /// Handle a document being opened: store text, parse, and index.
    pub fn open_document(&mut self, uri: DocumentUri, text: String) {
        let index = Self::build_index(&text);
        self.documents.insert(uri.as_str().to_string(), text);
        self.realm.add_document(uri, index);
    }

    /// Handle a document being changed: apply changes, re-parse, re-index.
    pub fn change_document(&mut self, uri: &DocumentUri, text: String) {
        self.realm.remove_document(uri);
        let index = Self::build_index(&text);
        self.documents.insert(uri.as_str().to_string(), text);
        self.realm.add_document(uri.clone(), index);
    }

    /// Handle a document being closed: remove from store and index.
    pub fn close_document(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri.as_str());
        self.realm.remove_document(uri);
    }

    /// Get the stored text for a document.
    pub fn get_document_text(&self, uri: &DocumentUri) -> Option<&str> {
        self.documents.get(uri.as_str()).map(|s| s.as_str())
    }

    /// Get the document index for a URI.
    pub fn get_document_index(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.realm.get_document(uri)
    }

    /// Get a reference to the realm index.
    pub fn realm(&self) -> &RealmIndex {
        &self.realm
    }

    /// Get the number of open documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }
}
