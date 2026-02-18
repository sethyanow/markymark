//! Server state: document store, parsing, and indexing.

pub mod completion;
pub mod navigation;
pub mod rename;

use std::collections::HashMap;

use markymark_core::structured::DocumentKind;
use markymark_core::DocumentUri;
use markymark_index::{
    AnyDocumentIndex, BlockOwned, DocumentIndex, MarkdownLinkOwned, RealmIndex,
    StructuredDocumentIndex, WikiLinkOwned, XmlTagOwned,
};
use markymark_parser::structured::parse_structured;
use markymark_parser::{byte_to_point, InputEdit, MarkdownTree, Parser};

pub use crate::diagnostics::{DiagnosticSeverity, MarkyDiagnostic};
use crate::incremental::{self, incremental_byte_bounds};
pub use completion::{CompletionCandidate, CompletionCandidateKind, CompletionContext};
pub use navigation::{StructuredKeyInfo, SymbolAtPosition};
pub use rename::{PrepareRenameResult, RenameEdit};

/// A content change event representing either a full document replacement
/// or an incremental text edit with LSP positions.
#[derive(Debug, Clone)]
pub enum DocumentChange {
    /// Full document text replacement.
    Full(String),
    /// Incremental edit specified with LSP line/character positions.
    /// Character offsets are in UTF-16 code units (as per LSP spec).
    Incremental {
        /// Start line (0-based).
        start_line: u32,
        /// Start character offset (UTF-16 code units).
        start_character: u32,
        /// End line (0-based).
        end_line: u32,
        /// End character offset (UTF-16 code units).
        end_character: u32,
        /// The replacement text.
        text: String,
    },
}

/// The internal state of the LSP server.
///
/// Manages document text storage, parsed ASTs, and the realm index.
/// The parser is stored here to avoid re-creating it on every parse call.
/// The `MarkdownTree` per document is retained for future incremental parsing.
pub struct ServerState {
    /// Raw document text keyed by URI string.
    documents: HashMap<String, String>,
    /// The realm index for cross-document lookups.
    realm: RealmIndex,
    /// Reusable markdown parser instance.
    parser: Parser,
    /// Retained tree-sitter parse trees for incremental reuse (keyed by URI string).
    md_trees: HashMap<String, MarkdownTree>,
    /// Pending incremental edits collected between change application and reindex.
    pending_edits: Vec<InputEdit>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    /// Create a new empty server state.
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            realm: RealmIndex::default(),
            parser: Parser::new().expect("failed to create parser"),
            md_trees: HashMap::new(),
            pending_edits: Vec::new(),
        }
    }

    /// Parse text and build a markdown document index.
    ///
    /// Returns both the index and the tree-sitter parse tree. The tree is
    /// retained per-document for future incremental parsing.
    fn build_markdown_index(&mut self, text: &str) -> (DocumentIndex, Option<MarkdownTree>) {
        self.build_markdown_index_with_old_tree(text, None, &[], None, None, None, None)
    }

    /// Parse text with optional old tree reuse and build a markdown document index.
    ///
    /// Delegates to [`incremental::build_markdown_index_with_old_tree`] which handles
    /// all 5 independent extractors (wiki_links, blocks, tags, markdown_links, xml_tags).
    #[allow(clippy::too_many_arguments)]
    fn build_markdown_index_with_old_tree(
        &mut self,
        text: &str,
        old_tree: Option<&MarkdownTree>,
        pending_edits: &[InputEdit],
        old_wiki_links: Option<&[WikiLinkOwned]>,
        old_blocks: Option<&[BlockOwned]>,
        old_markdown_links: Option<&[MarkdownLinkOwned]>,
        old_xml_tags: Option<&[XmlTagOwned]>,
    ) -> (DocumentIndex, Option<MarkdownTree>) {
        incremental::build_markdown_index_with_old_tree(
            &mut self.parser,
            text,
            old_tree,
            pending_edits,
            old_wiki_links,
            old_blocks,
            old_markdown_links,
            old_xml_tags,
        )
    }

    /// Detect document kind from URI file extension.
    fn document_kind_from_uri(uri: &DocumentUri) -> Option<DocumentKind> {
        uri.to_file_path()
            .as_deref()
            .and_then(DocumentKind::from_path)
    }

    /// Handle a document being opened: store text, parse, and index.
    pub fn open_document(&mut self, uri: DocumentUri, text: String) {
        let kind = Self::document_kind_from_uri(&uri);
        self.documents
            .insert(uri.as_str().to_string(), text.clone());

        match kind {
            Some(DocumentKind::Markdown) | None => {
                let (index, md_tree) = self.build_markdown_index(&text);
                if let Some(tree) = md_tree {
                    self.md_trees.insert(uri.as_str().to_string(), tree);
                }
                self.realm.add_document(uri, index);
            }
            Some(kind) => {
                if let Ok(ast) = parse_structured(&text, kind) {
                    self.realm
                        .add_structured_document(uri, StructuredDocumentIndex::from_ast(ast));
                }
            }
        }
    }

    /// Handle a document being changed: apply changes, re-parse, re-index.
    pub fn change_document(&mut self, uri: &DocumentUri, text: String) {
        self.pending_edits.clear();
        self.realm.remove_document(uri);
        let kind = Self::document_kind_from_uri(uri);
        self.documents
            .insert(uri.as_str().to_string(), text.clone());

        match kind {
            Some(DocumentKind::Markdown) | None => {
                let (index, md_tree) = self.build_markdown_index(&text);
                let uri_str = uri.as_str().to_string();
                if let Some(tree) = md_tree {
                    self.md_trees.insert(uri_str, tree);
                } else {
                    self.md_trees.remove(&uri_str);
                }
                self.realm.add_document(uri.clone(), index);
            }
            Some(kind) => {
                if let Ok(ast) = parse_structured(&text, kind) {
                    self.realm.add_structured_document(
                        uri.clone(),
                        StructuredDocumentIndex::from_ast(ast),
                    );
                }
            }
        }
    }

    /// Apply a sequence of content changes to a document, then re-parse and re-index.
    ///
    /// Changes are applied in order. Each change operates on the text as modified
    /// by the previous change (per LSP spec). Supports both incremental edits
    /// (with position range in UTF-16 code units) and full-text replacements.
    ///
    /// For markdown documents, incremental changes are tracked as tree-sitter
    /// `InputEdit`s so the old parse tree can be reused for O(edit_size) reparsing.
    pub fn apply_document_changes(&mut self, uri: &DocumentUri, changes: Vec<DocumentChange>) {
        self.pending_edits.clear();

        // Take the old tree out (if any) for incremental parsing
        let mut old_tree = self.md_trees.remove(uri.as_str());
        // Capture all old extractor data in a single get_document() call
        // before realm.remove_document() invalidates the arena.
        let (old_wiki_links, old_blocks, old_markdown_links, old_xml_tags) =
            if let Some(index) = self.realm.get_document(uri) {
                let wl = index
                    .wiki_links()
                    .iter()
                    .map(|entry| WikiLinkOwned {
                        target: entry.target.to_string(),
                        alias: entry.alias.map(str::to_string),
                        heading: entry.heading.map(str::to_string),
                        range: entry.range,
                        start_byte: entry.start_byte,
                        end_byte: entry.end_byte,
                    })
                    .collect::<Vec<_>>();
                let bl = index
                    .block_ids()
                    .filter_map(|id| index.block_by_id(id))
                    .map(|entry| BlockOwned {
                        id: entry.id.to_string(),
                        range: entry.range,
                        start_byte: entry.start_byte,
                        end_byte: entry.end_byte,
                    })
                    .collect::<Vec<_>>();
                let ml = index
                    .markdown_links()
                    .iter()
                    .map(|entry| MarkdownLinkOwned {
                        text: entry.text.to_string(),
                        url: entry.url.to_string(),
                        anchor: entry.anchor.map(str::to_string),
                        range: entry.range,
                        start_byte: entry.start_byte,
                        end_byte: entry.end_byte,
                    })
                    .collect::<Vec<_>>();
                let xt = index
                    .xml_tags()
                    .iter()
                    .map(|entry| XmlTagOwned {
                        tag_name: entry.tag_name.to_string(),
                        attributes: entry
                            .attributes
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect(),
                        is_self_closing: entry.is_self_closing,
                        is_unclosed: entry.is_unclosed,
                        range: entry.range,
                        start_byte: entry.start_byte,
                        end_byte: entry.end_byte,
                    })
                    .collect::<Vec<_>>();
                (Some(wl), Some(bl), Some(ml), Some(xt))
            } else {
                (None, None, None, None)
            };

        if changes.is_empty() {
            if let Some(tree) = old_tree {
                self.md_trees.insert(uri.as_str().to_string(), tree);
            }
            return;
        }

        // Phase 1: Apply text edits and track tree-sitter InputEdits
        let final_text = {
            let Some(text) = self.documents.get_mut(uri.as_str()) else {
                return;
            };

            for change in changes {
                match change {
                    DocumentChange::Full(new_text) => {
                        *text = new_text;
                        // Full replacement invalidates the old tree
                        old_tree = None;
                    }
                    DocumentChange::Incremental {
                        start_line,
                        start_character,
                        end_line,
                        end_character,
                        text: new_text,
                    } => {
                        let bounds = incremental_byte_bounds(
                            text,
                            start_line,
                            start_character,
                            end_line,
                            end_character,
                        );

                        if bounds.end_before_start {
                            eprintln!(
                                "markymark-lsp: skipping invalid incremental edit for {} \
                                 (old_end < start: start={}:{}, end={}:{})",
                                uri.as_str(),
                                start_line,
                                start_character,
                                end_line,
                                end_character
                            );
                            continue;
                        }

                        let start_byte = bounds.start_byte;
                        let old_end_byte = bounds.old_end_byte;

                        if bounds.start_clamped || bounds.end_clamped {
                            eprintln!(
                                "markymark-lsp: clamped incremental edit range for {} \
                                 (start={start_line}:{start_character}, end={end_line}:{end_character}, text_len_bytes={})",
                                uri.as_str(),
                                text.len()
                            );
                        }

                        // Compute tree-sitter Points BEFORE applying the text change
                        let start_position = byte_to_point(text, start_byte);
                        let old_end_position = byte_to_point(text, old_end_byte);

                        let new_end_byte = start_byte + new_text.len();

                        // Apply the text change
                        text.replace_range(start_byte..old_end_byte, &new_text);

                        // Compute new end position from the modified text
                        let new_end_position = byte_to_point(text, new_end_byte);

                        // Update the old tree so tree-sitter can reuse unchanged subtrees
                        let input_edit = InputEdit {
                            start_byte,
                            old_end_byte,
                            new_end_byte,
                            start_position,
                            old_end_position,
                            new_end_position,
                        };
                        self.pending_edits.push(input_edit);

                        if let Some(ref mut tree) = old_tree {
                            tree.edit(&input_edit);
                        }
                    }
                }
            }
            text.clone()
        };

        // Phase 2: Re-parse and re-index with the final text
        self.realm.remove_document(uri);
        let kind = Self::document_kind_from_uri(uri);

        match kind {
            Some(DocumentKind::Markdown) | None => {
                let pending_edits = self.pending_edits.clone();
                let (index, md_tree) = self.build_markdown_index_with_old_tree(
                    &final_text,
                    old_tree.as_ref(),
                    &pending_edits,
                    old_wiki_links.as_deref(),
                    old_blocks.as_deref(),
                    old_markdown_links.as_deref(),
                    old_xml_tags.as_deref(),
                );
                let uri_str = uri.as_str().to_string();
                if let Some(tree) = md_tree {
                    self.md_trees.insert(uri_str, tree);
                } else {
                    self.md_trees.remove(&uri_str);
                }
                self.realm.add_document(uri.clone(), index);
                self.pending_edits.clear();
            }
            Some(kind) => {
                if let Ok(ast) = parse_structured(&final_text, kind) {
                    self.realm.add_structured_document(
                        uri.clone(),
                        StructuredDocumentIndex::from_ast(ast),
                    );
                }
                self.pending_edits.clear();
            }
        }
    }

    /// Handle a document being closed: remove from store and index.
    pub fn close_document(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri.as_str());
        self.md_trees.remove(uri.as_str());
        self.realm.remove_document(uri);
        self.pending_edits.clear();
    }

    /// Get the stored text for a document.
    pub fn get_document_text(&self, uri: &DocumentUri) -> Option<&str> {
        self.documents.get(uri.as_str()).map(|s| s.as_str())
    }

    /// Get the markdown document index for a URI.
    pub fn get_document_index(&self, uri: &DocumentUri) -> Option<&DocumentIndex> {
        self.realm.get_document(uri)
    }

    /// Get the any-type document index for a URI.
    pub fn get_any_document_index(&self, uri: &DocumentUri) -> Option<&AnyDocumentIndex> {
        self.realm.get_any_document(uri)
    }

    /// Get the structured document index for a URI.
    pub fn get_structured_document_index(
        &self,
        uri: &DocumentUri,
    ) -> Option<&StructuredDocumentIndex> {
        self.realm.get_structured_document(uri)
    }

    /// Get the retained tree-sitter parse tree for a document.
    ///
    /// Used for incremental parsing: pass the old tree to `Parser::parse_incremental`
    /// so tree-sitter can reuse unchanged subtrees.
    pub fn get_md_tree(&self, uri: &DocumentUri) -> Option<&MarkdownTree> {
        self.md_trees.get(uri.as_str())
    }

    /// Number of pending incremental edits awaiting reindex.
    pub fn pending_edit_count(&self) -> usize {
        self.pending_edits.len()
    }

    /// Get a reference to the realm index.
    pub fn realm(&self) -> &RealmIndex {
        &self.realm
    }

    /// Get the number of open documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Compute diagnostics for a document.
    ///
    /// Checks for:
    /// - Broken wiki links (target page or heading doesn't exist)
    /// - Broken markdown link anchors (heading slug doesn't exist in current doc)
    /// - Duplicate heading slugs within the same document
    pub fn compute_diagnostics(&self, uri: &DocumentUri) -> Vec<MarkyDiagnostic> {
        let index = match self.realm.get_document(uri) {
            Some(idx) => idx,
            None => return Vec::new(),
        };
        crate::diagnostics::compute_diagnostics(index, &self.realm, uri)
    }
}
