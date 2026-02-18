//! Server state: document store, parsing, and indexing.

pub mod completion;

use std::collections::HashMap;

use markymark_core::structured::{DocumentKind, KeyEntry, ValueKind};
use markymark_core::{DocumentUri, Position, Range};
use markymark_index::{
    slugify, AnyDocumentIndex, BlockOwned, DocumentIndex, HeadingEntry, MarkdownLinkEntry,
    MarkdownLinkOwned, RealmIndex, StructuredDocumentIndex, WikiLinkEntry, WikiLinkOwned,
    XmlTagEntry, XmlTagOwned,
};
use markymark_parser::structured::parse_structured;
use markymark_parser::{byte_to_point, InputEdit, MarkdownTree, Parser};

pub use crate::diagnostics::{DiagnosticSeverity, MarkyDiagnostic};
use crate::incremental::{self, incremental_byte_bounds};
pub use completion::{CompletionCandidate, CompletionCandidateKind, CompletionContext};

/// Result from `prepare_rename_at`: the range and current text of the renameable symbol.
#[derive(Debug, Clone)]
pub struct PrepareRenameResult {
    /// The source range of the renameable text.
    pub range: Range,
    /// The current text (used as placeholder in rename dialog).
    pub placeholder: String,
}

/// A single text edit produced by a rename operation.
#[derive(Debug, Clone)]
pub struct RenameEdit {
    /// The document to edit.
    pub uri: DocumentUri,
    /// The range to replace.
    pub range: Range,
    /// The replacement text.
    pub new_text: String,
}

/// Describes what symbol (if any) the cursor is sitting on.
#[derive(Debug, Clone)]
pub enum SymbolAtPosition<'a> {
    /// A heading line.
    Heading(HeadingEntry<'a>),
    /// A wiki link.
    WikiLink(WikiLinkEntry<'a>),
    /// A markdown link.
    MarkdownLink(MarkdownLinkEntry<'a>),
    /// An XML tag.
    XmlTag(XmlTagEntry<'a>),
    /// A key in a structured document (JSON, YAML, TOML, etc.).
    StructuredKey(StructuredKeyInfo),
}

/// Information about a structured document key at the cursor position.
#[derive(Debug, Clone)]
pub struct StructuredKeyInfo {
    /// Full dotted key path (e.g. `"database.host"`).
    pub path: String,
    /// Leaf key name (e.g. `"host"`).
    pub key: String,
    /// Nesting depth (0 = top-level).
    pub depth: usize,
    /// Classification of the value.
    pub value_kind: ValueKind,
    /// The document kind (Json, Yaml, Toml, etc.).
    pub document_kind: DocumentKind,
}

impl StructuredKeyInfo {
    /// Build from a [`KeyEntry`] and the document kind.
    pub fn from_key_entry(entry: &KeyEntry, kind: DocumentKind) -> Self {
        Self {
            path: entry.path.clone(),
            key: entry.key.clone(),
            depth: entry.depth,
            value_kind: entry.value_kind,
            document_kind: kind,
        }
    }
}

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

    /// Check whether the symbol at the given position can be renamed.
    ///
    /// Returns the range and current text of the renameable symbol, or `None`
    /// if no renameable symbol is found at the position.
    pub fn prepare_rename_at(
        &self,
        uri: &DocumentUri,
        pos: Position,
    ) -> Option<PrepareRenameResult> {
        let symbol = self.symbol_at_position(uri, pos)?;
        match symbol {
            SymbolAtPosition::Heading(h) => Some(PrepareRenameResult {
                range: h.range,
                placeholder: h.text.to_string(),
            }),
            SymbolAtPosition::XmlTag(xt) => {
                // Tag name range: starts after '<', length of tag_name
                let name_start = Position::new(xt.range.start.line, xt.range.start.character + 1);
                let name_end = Position::new(
                    xt.range.start.line,
                    xt.range.start.character + 1 + xt.tag_name.len() as u32,
                );
                Some(PrepareRenameResult {
                    range: Range::new(name_start, name_end),
                    placeholder: xt.tag_name.to_string(),
                })
            }
            // Wiki links and markdown links are not renameable themselves
            // (you rename the heading they point to, not the link)
            _ => None,
        }
    }

    /// Compute all edits needed to rename the symbol at the given position.
    ///
    /// For a heading rename, this:
    /// 1. Renames the heading text in the source document
    /// 2. Updates all wiki links that reference the old heading slug
    /// 3. Updates all markdown link anchors (`#old-slug` → `#new-slug`)
    pub fn rename_at(
        &self,
        uri: &DocumentUri,
        pos: Position,
        new_name: &str,
    ) -> Option<Vec<RenameEdit>> {
        let symbol = self.symbol_at_position(uri, pos)?;
        match symbol {
            SymbolAtPosition::Heading(h) => {
                let old_slug = h.slug;
                let new_slug = slugify(new_name);
                let mut edits = Vec::new();

                // 1. Edit the heading text itself.
                //    The heading range covers the full line including `# ` prefix.
                //    We need to compute the text-only range: skip "## " prefix.
                let text = self.get_document_text(uri)?;
                let heading_line = text.lines().nth(h.range.start.line as usize)?;
                let prefix_len =
                    heading_line.len() - heading_line.trim_start_matches('#').trim_start().len();
                let text_start = Position::new(h.range.start.line, prefix_len as u32);
                let text_end =
                    Position::new(h.range.start.line, prefix_len as u32 + h.text.len() as u32);
                edits.push(RenameEdit {
                    uri: uri.clone(),
                    range: Range::new(text_start, text_end),
                    new_text: new_name.to_string(),
                });

                // 2. Search all documents for wiki links referencing the old slug
                for (doc_uri, index) in self.realm.iter_documents() {
                    for wl in index.wiki_links() {
                        if wl.heading == Some(old_slug) {
                            // Compute the range of just the heading part in the wiki link.
                            // Wiki link format: [[target#heading]] or [[#heading]]
                            // We need to replace just the heading text after #.
                            let doc_text = self.get_document_text(doc_uri);
                            if let Some(anchor_range) =
                                find_wiki_link_heading_range(doc_text, wl, old_slug)
                            {
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: anchor_range,
                                    new_text: new_name.to_string(),
                                });
                            }
                        }
                    }

                    // 3. Update markdown link anchors: [text](#old-slug) → [text](#new-slug)
                    for ml in index.markdown_links() {
                        if ml.anchor == Some(old_slug) {
                            let doc_text = self.get_document_text(doc_uri);
                            if let Some(anchor_range) =
                                find_markdown_link_anchor_range(doc_text, ml, old_slug)
                            {
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: anchor_range,
                                    new_text: new_slug.clone(),
                                });
                            }
                        }
                    }
                }

                Some(edits)
            }
            SymbolAtPosition::XmlTag(xt) => {
                let old_name = &xt.tag_name;
                let mut edits = Vec::new();

                // Find all XML tags with the same name across all documents
                for (doc_uri, index) in self.realm.iter_documents() {
                    for xml in index.xml_tags() {
                        if xml.tag_name == *old_name {
                            // Opening tag name: starts after '<', length of tag_name
                            let name_start =
                                Position::new(xml.range.start.line, xml.range.start.character + 1);
                            let name_end = Position::new(
                                xml.range.start.line,
                                xml.range.start.character + 1 + xml.tag_name.len() as u32,
                            );
                            edits.push(RenameEdit {
                                uri: doc_uri.clone(),
                                range: Range::new(name_start, name_end),
                                new_text: new_name.to_string(),
                            });

                            // Closing tag name: ends just before '>' in </tagname>
                            if !xml.is_self_closing && !xml.is_unclosed {
                                let close_name_start = Position::new(
                                    xml.range.end.line,
                                    xml.range.end.character - 1 - xml.tag_name.len() as u32,
                                );
                                let close_name_end =
                                    Position::new(xml.range.end.line, xml.range.end.character - 1);
                                edits.push(RenameEdit {
                                    uri: doc_uri.clone(),
                                    range: Range::new(close_name_start, close_name_end),
                                    new_text: new_name.to_string(),
                                });
                            }
                        }
                    }
                }

                if edits.is_empty() {
                    None
                } else {
                    Some(edits)
                }
            }
            _ => None,
        }
    }

    /// Identify what element the cursor is on.
    pub fn symbol_at_position(
        &self,
        uri: &DocumentUri,
        pos: Position,
    ) -> Option<SymbolAtPosition<'_>> {
        // Check if it's a structured document first
        if let Some(structured_index) = self.realm.get_structured_document(uri) {
            // Find the key entry whose key_range contains the cursor position.
            // Iterate in reverse to prefer deeper (more specific) keys when nested.
            for entry in structured_index.keys().iter().rev() {
                if entry.key_range.contains(pos) {
                    return Some(SymbolAtPosition::StructuredKey(
                        StructuredKeyInfo::from_key_entry(entry, structured_index.kind()),
                    ));
                }
            }
            return None;
        }

        let index = self.realm.get_document(uri)?;

        // Check wiki links first (most specific)
        for wl in index.wiki_links() {
            if wl.range.contains(pos) {
                return Some(SymbolAtPosition::WikiLink(wl.clone()));
            }
        }

        // Check markdown links
        for ml in index.markdown_links() {
            if ml.range.contains(pos) {
                return Some(SymbolAtPosition::MarkdownLink(ml.clone()));
            }
        }

        // Check headings
        for h in index.headings() {
            if h.range.contains(pos) {
                return Some(SymbolAtPosition::Heading(h.clone()));
            }
        }

        // Check XML tags
        for xt in index.xml_tags() {
            if xt.range.contains(pos) {
                return Some(SymbolAtPosition::XmlTag(xt.clone()));
            }
        }

        None
    }
}

/// Find the range of the heading portion within a wiki link.
///
/// Given a wiki link like `[[page#heading]]` or `[[#heading]]`, returns the
/// range covering just the heading text (after `#`, before `]]`).
fn find_wiki_link_heading_range(
    doc_text: Option<&str>,
    wl: &WikiLinkEntry,
    old_heading: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(wl.range.start.line as usize)?;
    let link_start = wl.range.start.character as usize;
    let link_text = &line[link_start..];

    // Find `#heading` within the wiki link text
    let hash_offset = link_text.find('#')?;
    let heading_start = link_start + hash_offset + 1; // skip the '#'

    // Verify the text matches
    let heading_end = heading_start + old_heading.len();
    if line.get(heading_start..heading_end) == Some(old_heading) {
        Some(Range::new(
            Position::new(wl.range.start.line, heading_start as u32),
            Position::new(wl.range.start.line, heading_end as u32),
        ))
    } else {
        None
    }
}

/// Find the range of the anchor portion within a markdown link.
///
/// Given a markdown link like `[text](#slug)`, returns the range covering
/// just the slug text (after `#`, before `)`).
fn find_markdown_link_anchor_range(
    doc_text: Option<&str>,
    ml: &MarkdownLinkEntry,
    old_slug: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(ml.range.start.line as usize)?;
    let link_start = ml.range.start.character as usize;
    let link_text = &line[link_start..];

    // Find `(#slug)` within the markdown link text
    let paren_hash = link_text.find("(#")?;
    let slug_start = link_start + paren_hash + 2; // skip "(#"
    let slug_end = slug_start + old_slug.len();

    if line.get(slug_start..slug_end) == Some(old_slug) {
        Some(Range::new(
            Position::new(ml.range.start.line, slug_start as u32),
            Position::new(ml.range.start.line, slug_end as u32),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_byte_bounds_reports_clamp_when_position_exceeds_document() {
        let text = "# Title\n";
        let bounds = incremental_byte_bounds(text, 99, 99, 99, 120);
        assert_eq!(bounds.start_byte, text.len());
        assert_eq!(bounds.old_end_byte, text.len());
        assert!(bounds.start_clamped);
        assert!(bounds.end_clamped);
        assert!(!bounds.end_before_start);
    }

    #[test]
    fn test_incremental_byte_bounds_end_before_start() {
        let text = "line0\nline1\nline2\n";
        // end (line 0, char 2) is before start (line 1, char 3)
        let bounds = incremental_byte_bounds(text, 1, 3, 0, 2);
        assert!(
            bounds.end_before_start,
            "end position should be before start"
        );
        // old_end_byte is still coerced for consistency
        assert!(bounds.old_end_byte >= bounds.start_byte);
    }

    #[test]
    fn test_range_is_after_edit_start_spanning_link_returns_false() {
        // A link that STARTS before the edit but ENDS after it spans the edit.
        // range_intersects_edit handles spanning links; range_is_after_edit_start should NOT
        // additionally catch them — only links whose START is >= edit start are "after".
        let range = Range::new(Position::new(0, 0), Position::new(5, 0));
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 10,
            new_end_byte: 10,
            start_position: markymark_parser::Point { row: 2, column: 0 },
            old_end_position: markymark_parser::Point { row: 2, column: 10 },
            new_end_position: markymark_parser::Point { row: 2, column: 10 },
        };
        assert!(
            !incremental::range_is_after_edit_start(range, &edit),
            "a link starting before the edit should not be 'after edit start'"
        );
    }

    #[test]
    fn test_range_is_after_edit_start_link_after_edit_returns_true() {
        // A link entirely after the edit should be "after edit start".
        let range = Range::new(Position::new(5, 0), Position::new(7, 0));
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 5,
            new_end_byte: 5,
            start_position: markymark_parser::Point { row: 2, column: 0 },
            old_end_position: markymark_parser::Point { row: 2, column: 5 },
            new_end_position: markymark_parser::Point { row: 2, column: 5 },
        };
        assert!(
            incremental::range_is_after_edit_start(range, &edit),
            "a link starting after the edit should be 'after edit start'"
        );
    }

    #[test]
    fn test_range_within_neighbor_window_adjacent_bytes_is_in_window() {
        // A link starting 10 bytes after the edit end should be within a 100-byte window.
        // Uses byte offsets directly — works correctly across line boundaries.
        let edit = InputEdit {
            start_byte: 50,
            old_end_byte: 60,
            new_end_byte: 60,
            start_position: markymark_parser::Point { row: 5, column: 0 },
            old_end_position: markymark_parser::Point { row: 5, column: 10 },
            new_end_position: markymark_parser::Point { row: 5, column: 10 },
        };
        // Link at bytes 70–85, which is 10 bytes after the edit end (60).
        assert!(
            incremental::range_within_neighbor_window(70, 85, &edit, 100),
            "a link 10 bytes after the edit end should be within a 100-byte window"
        );
    }

    #[test]
    fn test_range_within_neighbor_window_far_link_not_in_window() {
        // A link 200 bytes away should not be within a 100-byte window.
        let edit = InputEdit {
            start_byte: 50,
            old_end_byte: 60,
            new_end_byte: 60,
            start_position: markymark_parser::Point { row: 5, column: 0 },
            old_end_position: markymark_parser::Point { row: 5, column: 10 },
            new_end_position: markymark_parser::Point { row: 5, column: 10 },
        };
        // Link at bytes 261–280, which is 201 bytes after the edit end (60).
        assert!(
            !incremental::range_within_neighbor_window(261, 280, &edit, 100),
            "a link 200 bytes from the edit should not be within a 100-byte window"
        );
    }

    #[test]
    fn test_wiki_links_need_update_for_edit_after_last_existing_link() {
        let old_wiki_links = vec![WikiLinkOwned {
            target: "Page".to_string(),
            alias: None,
            heading: None,
            range: Range::new(Position::new(1, 2), Position::new(1, 10)),
            start_byte: 10,
            end_byte: 18,
        }];
        let pending_edits = vec![InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 7,
            start_position: markymark_parser::Point { row: 3, column: 0 },
            old_end_position: markymark_parser::Point { row: 3, column: 0 },
            new_end_position: markymark_parser::Point { row: 3, column: 7 },
        }];

        assert!(
            incremental::wiki_links_need_update(&old_wiki_links, &pending_edits),
            "append edits after the last link should force wiki-link recomputation"
        );
    }

    // ---- Block incremental merge tests ----

    fn make_block_owned(
        id: &str,
        start_line: u32,
        start_col: u32,
        end_col: u32,
        start_byte: usize,
        end_byte: usize,
    ) -> BlockOwned {
        BlockOwned {
            id: id.to_string(),
            range: Range::new(
                Position::new(start_line, start_col),
                Position::new(start_line, end_col),
            ),
            start_byte,
            end_byte,
        }
    }

    #[test]
    fn test_blocks_need_update_returns_false_when_no_pending_edits() {
        let old_blocks = vec![make_block_owned("block-1", 2, 10, 18, 30, 38)];
        assert!(
            !incremental::blocks_need_update(&old_blocks, &[]),
            "empty pending_edits should not require block update"
        );
    }

    #[test]
    fn test_blocks_need_update_returns_true_for_intersecting_edit() {
        let old_blocks = vec![make_block_owned("block-1", 2, 10, 18, 30, 38)];
        // Edit overlaps the block range
        let edit = InputEdit {
            start_byte: 28,
            old_end_byte: 35,
            new_end_byte: 35,
            start_position: markymark_parser::Point { row: 2, column: 8 },
            old_end_position: markymark_parser::Point { row: 2, column: 15 },
            new_end_position: markymark_parser::Point { row: 2, column: 15 },
        };
        assert!(
            incremental::blocks_need_update(&old_blocks, &[edit]),
            "edit overlapping block range should require update"
        );
    }

    #[test]
    fn test_blocks_need_update_returns_false_for_pre_block_edit_no_neighbor() {
        // Edit at byte 0-1, block at bytes 500-508 (far beyond 100-byte neighbor window)
        let old_blocks = vec![make_block_owned("block-far", 10, 0, 8, 500, 508)];
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 1,
            new_end_byte: 1,
            start_position: markymark_parser::Point { row: 0, column: 0 },
            old_end_position: markymark_parser::Point { row: 0, column: 1 },
            new_end_position: markymark_parser::Point { row: 0, column: 1 },
        };
        // range_intersects_edit: false (no overlap)
        // range_is_after_edit_start: true (block at row 10 >= edit start row 0)
        // → affected because position shifted; blocks_need_update should return true
        assert!(
            incremental::blocks_need_update(&old_blocks, &[edit]),
            "edit before block shifts block position, requiring update"
        );
    }

    #[test]
    fn test_blocks_need_update_for_edit_at_or_after_last_block() {
        let old_blocks = vec![make_block_owned("block-1", 1, 2, 10, 10, 18)];
        // Edit starts at row 3 (after all blocks)
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 7,
            start_position: markymark_parser::Point { row: 3, column: 0 },
            old_end_position: markymark_parser::Point { row: 3, column: 0 },
            new_end_position: markymark_parser::Point { row: 3, column: 7 },
        };
        assert!(
            incremental::blocks_need_update(&old_blocks, &[edit]),
            "append edits after last block should force block recomputation"
        );
    }

    #[test]
    fn test_merge_incremental_blocks_reuses_unaffected_old_blocks() {
        // Block at row 5 (bytes 100-108), edit at row 0 (byte 0-1)
        // range_is_after_edit_start: true (row 5 >= row 0) → affected
        // So the block at row 5 is "affected" (its byte offset shifted) and comes from new.
        // A block at bytes < edit start would be unaffected.
        // Edit at row 5 col 50 (byte 200), block at row 0 col 10 (byte 10-18).
        let old_blocks = vec![make_block_owned("early-block", 0, 10, 18, 10, 18)];
        let new_blocks = vec![make_block_owned("early-block", 0, 10, 18, 10, 18)]; // same positions
        let edit = InputEdit {
            start_byte: 200,
            old_end_byte: 201,
            new_end_byte: 201,
            start_position: markymark_parser::Point { row: 5, column: 50 },
            old_end_position: markymark_parser::Point { row: 5, column: 51 },
            new_end_position: markymark_parser::Point { row: 5, column: 51 },
        };
        let merged = incremental::merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
        assert_eq!(merged.len(), 1, "merged should contain exactly one block");
        assert_eq!(merged[0].id, "early-block");
    }

    #[test]
    fn test_merge_incremental_blocks_deduplicates_when_both_contribute() {
        // Old has two blocks; edit is between them.
        // Block-A at row 0 (before edit) → unaffected → from old
        // Block-B at row 5 (after edit) → affected → from new
        let old_blocks = vec![
            make_block_owned("block-a", 0, 10, 18, 10, 18),
            make_block_owned("block-b", 5, 10, 18, 200, 208),
        ];
        let new_blocks = vec![
            // block-a unchanged
            make_block_owned("block-a", 0, 10, 18, 10, 18),
            // block-b has updated position after edit
            make_block_owned("block-b", 5, 10, 18, 201, 209),
        ];
        // Edit at row 3 (between the two blocks)
        let edit = InputEdit {
            start_byte: 100,
            old_end_byte: 100,
            new_end_byte: 101, // insert 1 byte
            start_position: markymark_parser::Point { row: 3, column: 0 },
            old_end_position: markymark_parser::Point { row: 3, column: 0 },
            new_end_position: markymark_parser::Point { row: 3, column: 1 },
        };
        let merged = incremental::merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
        // Both blocks should appear exactly once
        assert_eq!(merged.len(), 2, "merged should contain exactly two blocks");
        assert!(merged.iter().any(|b| b.id == "block-a"));
        assert!(merged.iter().any(|b| b.id == "block-b"));
    }

    #[test]
    fn test_build_markdown_index_incremental_blocks_parity() {
        // Build a document, apply a character insertion far from blocks,
        // verify incremental block result matches full rebuild.
        use markymark_parser::Parser;

        let original = "# Title\n\nSome text far from blocks.\n\nBlock here ^my-block\n\nAnother ^other-block\n";
        let mut parser = Parser::new().unwrap();

        // Initial parse
        let ast0 = parser.parse(original).unwrap();
        let index0 = DocumentIndex::from_ast(ast0);
        let old_block_ids: Vec<String> = index0.block_ids().map(str::to_string).collect();

        // Single-char insertion at start of title line
        let edit_text = "A";
        let modified = format!("A{original}");

        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 1,
            start_position: markymark_parser::Point { row: 0, column: 0 },
            old_end_position: markymark_parser::Point { row: 0, column: 0 },
            new_end_position: markymark_parser::Point { row: 0, column: 1 },
        };

        // Build expected full rebuild
        let ast_full = parser.parse(&modified).unwrap();
        let full_index = DocumentIndex::from_ast(ast_full);
        let full_block_ids: Vec<String> = full_index.block_ids().map(str::to_string).collect();

        // Build old blocks owned (simulate what apply_document_changes captures)
        let old_blocks_owned: Vec<BlockOwned> = index0
            .block_ids()
            .filter_map(|id| index0.block_by_id(id))
            .map(|entry| BlockOwned {
                id: entry.id.to_string(),
                range: entry.range,
                start_byte: entry.start_byte,
                end_byte: entry.end_byte,
            })
            .collect();

        // Incremental rebuild
        let ast_inc = parser.parse(&modified).unwrap();
        let inc_index = incremental::build_markdown_index_incremental(
            ast_inc,
            &[edit],
            None,
            Some(&old_blocks_owned),
            None,
            None,
        );
        let inc_block_ids: Vec<String> = inc_index.block_ids().map(str::to_string).collect();

        let mut full_sorted = full_block_ids.clone();
        let mut inc_sorted = inc_block_ids.clone();
        full_sorted.sort();
        inc_sorted.sort();
        assert_eq!(
            full_sorted, inc_sorted,
            "incremental block IDs should match full rebuild: full={full_block_ids:?} inc={inc_block_ids:?}"
        );

        let _ = (edit_text, old_block_ids);
    }
}
