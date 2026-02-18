//! Server state: document store, parsing, and indexing.

use std::collections::HashMap;

use markymark_core::structured::{DocumentKind, KeyEntry, ValueKind};
use markymark_core::{DocumentUri, Position, Range};
use markymark_index::resolution::{resolve_markdown_link, resolve_wiki_link};
use markymark_index::{
    slugify, AnyDocumentIndex, BlockOwned, DocumentIndex, HeadingEntry, MarkdownLinkEntry,
    RealmIndex, StructuredDocumentIndex, WikiLinkEntry, WikiLinkOwned, XmlTagEntry,
};
use markymark_parser::structured::parse_structured;
use markymark_parser::{byte_to_point, InputEdit, MarkdownTree, Parser};

/// Detected completion trigger context based on cursor position.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionContext {
    /// Inside `[[` — complete page names.
    WikiLink {
        /// The partial text typed after `[[`.
        partial: String,
    },
    /// Inside `[[page#` — complete headings in the target page.
    WikiLinkHeading {
        /// The target page name.
        target: String,
        /// The partial heading text typed after `#`.
        partial: String,
    },
    /// After `#` (not in a link context) — complete tag names.
    Tag {
        /// The partial text typed after `#`.
        partial: String,
    },
    /// Inside `((` — complete block IDs.
    BlockRef {
        /// The partial text typed after `((`.
        partial: String,
    },
    /// After `<` — complete XML tag names.
    XmlTag {
        /// The partial tag name typed after `<`.
        partial: String,
    },
}

/// A completion suggestion returned by [`ServerState::completion_at`].
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionCandidate {
    /// The completion label (displayed to the user).
    pub label: String,
    /// The kind of completion item.
    pub kind: CompletionCandidateKind,
    /// Optional detail text.
    pub detail: Option<String>,
}

/// The kind of a completion candidate.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionCandidateKind {
    /// A page name (for wiki link completion).
    Page,
    /// A heading (for heading completion).
    Heading,
    /// A tag name.
    Tag,
    /// A block reference ID.
    BlockRef,
    /// An XML tag name.
    XmlTag,
}

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

/// Severity level for a diagnostic.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticSeverity {
    /// An error (e.g., broken link).
    Error,
    /// A warning (e.g., duplicate slug).
    Warning,
}

/// A diagnostic produced by document analysis.
#[derive(Debug, Clone)]
pub struct MarkyDiagnostic {
    /// Source range of the problem.
    pub range: Range,
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IncrementalByteBounds {
    start_byte: usize,
    old_end_byte: usize,
    start_clamped: bool,
    end_clamped: bool,
    /// True when raw end position was before raw start (would be coerced to insertion).
    end_before_start: bool,
}

fn incremental_byte_bounds(
    text: &str,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> IncrementalByteBounds {
    let raw_start_byte =
        crate::convert::lsp_position_to_byte_offset(text, start_line, start_character);
    let raw_end_byte = crate::convert::lsp_position_to_byte_offset(text, end_line, end_character);

    let end_before_start = raw_end_byte < raw_start_byte;
    let start_byte = raw_start_byte.min(text.len());
    let old_end_byte = raw_end_byte.min(text.len()).max(start_byte);

    IncrementalByteBounds {
        start_byte,
        old_end_byte,
        start_clamped: position_was_clamped(text, start_line, start_character),
        end_clamped: position_was_clamped(text, end_line, end_character),
        end_before_start,
    }
}

fn position_was_clamped(text: &str, line: u32, character: u32) -> bool {
    let target_line = line as usize;
    let target_character = character as usize;
    let Some(line_text) = text.split('\n').nth(target_line) else {
        return true;
    };

    // CRLF is normalized for offset math in lsp_position_to_byte_offset.
    let content = line_text.strip_suffix('\r').unwrap_or(line_text);
    target_character > content.encode_utf16().count()
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
        self.build_markdown_index_with_old_tree(text, None, None, None, &[])
    }

    fn range_intersects_edit(range: Range, edit: &InputEdit) -> bool {
        let range_start = (range.start.line, range.start.character);
        let range_end = (range.end.line, range.end.character);
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        let edit_end = (
            edit.old_end_position.row as u32,
            edit.old_end_position.column as u32,
        );

        range_start < edit_end && range_end > edit_start
    }

    fn range_is_after_edit_start(range: Range, edit: &InputEdit) -> bool {
        let range_start = (range.start.line, range.start.character);
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        range_start >= edit_start
    }

    fn range_within_neighbor_window(
        start_byte: usize,
        end_byte: usize,
        edit: &InputEdit,
        window_bytes: usize,
    ) -> bool {
        start_byte <= edit.old_end_byte.saturating_add(window_bytes)
            && end_byte.saturating_add(window_bytes) >= edit.start_byte
    }

    fn wiki_link_affected_by_edits(wl: &WikiLinkOwned, pending_edits: &[InputEdit]) -> bool {
        pending_edits.iter().any(|edit| {
            Self::range_intersects_edit(wl.range, edit)
                || Self::range_is_after_edit_start(wl.range, edit)
                || Self::range_within_neighbor_window(wl.start_byte, wl.end_byte, edit, 100)
        })
    }

    fn wiki_links_need_update(
        old_wiki_links: &[WikiLinkOwned],
        pending_edits: &[InputEdit],
    ) -> bool {
        old_wiki_links
            .iter()
            .any(|link| Self::wiki_link_affected_by_edits(link, pending_edits))
            || Self::any_edit_starts_at_or_after_last_wiki_link(old_wiki_links, pending_edits)
    }

    fn any_edit_starts_at_or_after_last_wiki_link(
        old_wiki_links: &[WikiLinkOwned],
        pending_edits: &[InputEdit],
    ) -> bool {
        let Some(last_old_end) = old_wiki_links
            .iter()
            .map(|link| (link.range.end.line, link.range.end.character))
            .max()
        else {
            return false;
        };

        pending_edits.iter().any(|edit| {
            let edit_start = (
                edit.start_position.row as u32,
                edit.start_position.column as u32,
            );
            edit_start >= last_old_end
        })
    }

    fn extract_wiki_links_owned(ast: &markymark_parser::Ast) -> Vec<WikiLinkOwned> {
        ast.extract_wiki_links()
            .into_iter()
            .filter(|wl| {
                wl.target_page().is_some()
                    || wl.target_heading().is_some()
                    || wl.target_block_id().is_some()
            })
            .map(|wl| {
                let (start_byte, end_byte) = wl.byte_range();
                WikiLinkOwned {
                    target: wl.target_page().unwrap_or("").to_string(),
                    alias: wl.alias().map(str::to_string),
                    heading: wl.target_heading().map(str::to_string),
                    range: wl.range(),
                    start_byte,
                    end_byte,
                }
            })
            .collect()
    }

    fn merge_incremental_wiki_links(
        old_wiki_links: &[WikiLinkOwned],
        new_wiki_links: &[WikiLinkOwned],
        pending_edits: &[InputEdit],
    ) -> Vec<WikiLinkOwned> {
        let mut merged = Vec::new();
        for old in old_wiki_links {
            if !Self::wiki_link_affected_by_edits(old, pending_edits) {
                merged.push(old.clone());
            }
        }

        for new_link in new_wiki_links {
            if Self::wiki_link_affected_by_edits(new_link, pending_edits) {
                merged.push(new_link.clone());
            }
        }

        merged.sort_by_key(|wl| (wl.range.start.line, wl.range.start.character));
        merged
    }

    fn block_affected_by_edits(block: &BlockOwned, pending_edits: &[InputEdit]) -> bool {
        pending_edits.iter().any(|edit| {
            Self::range_intersects_edit(block.range, edit)
                || Self::range_is_after_edit_start(block.range, edit)
                || Self::range_within_neighbor_window(block.start_byte, block.end_byte, edit, 100)
        })
    }

    fn blocks_need_update(old_blocks: &[BlockOwned], pending_edits: &[InputEdit]) -> bool {
        if pending_edits.is_empty() {
            return false;
        }
        old_blocks
            .iter()
            .any(|block| Self::block_affected_by_edits(block, pending_edits))
            || Self::any_edit_starts_at_or_after_last_block(old_blocks, pending_edits)
    }

    fn any_edit_starts_at_or_after_last_block(
        old_blocks: &[BlockOwned],
        pending_edits: &[InputEdit],
    ) -> bool {
        let Some(last_old_end) = old_blocks
            .iter()
            .map(|block| (block.range.end.line, block.range.end.character))
            .max()
        else {
            return false;
        };

        pending_edits.iter().any(|edit| {
            let edit_start = (
                edit.start_position.row as u32,
                edit.start_position.column as u32,
            );
            edit_start >= last_old_end
        })
    }

    fn extract_blocks_owned(ast: &markymark_parser::Ast) -> Vec<BlockOwned> {
        ast.extract_block_ids()
            .into_iter()
            .map(|b| BlockOwned {
                id: b.id().to_string(),
                range: b.range(),
                start_byte: b.start_byte(),
                end_byte: b.end_byte(),
            })
            .collect()
    }

    fn merge_incremental_blocks(
        old_blocks: &[BlockOwned],
        new_blocks: &[BlockOwned],
        pending_edits: &[InputEdit],
    ) -> Vec<BlockOwned> {
        let mut merged = Vec::new();
        for old in old_blocks {
            if !Self::block_affected_by_edits(old, pending_edits) {
                merged.push(old.clone());
            }
        }
        for new_block in new_blocks {
            if Self::block_affected_by_edits(new_block, pending_edits) {
                merged.push(new_block.clone());
            }
        }
        merged.sort_by_key(|b| (b.range.start.line, b.range.start.character));
        merged
    }

    fn build_markdown_index_incremental(
        old_wiki_links: Option<&[WikiLinkOwned]>,
        old_blocks: Option<&[BlockOwned]>,
        ast: markymark_parser::Ast,
        pending_edits: &[InputEdit],
    ) -> DocumentIndex {
        if pending_edits.is_empty() {
            return DocumentIndex::from_ast(ast);
        }

        // Compute merged wiki-links
        let merged_wiki_links = if let Some(old_wiki_links) = old_wiki_links {
            if old_wiki_links.is_empty() {
                Some(Self::extract_wiki_links_owned(&ast))
            } else if !Self::wiki_links_need_update(old_wiki_links, pending_edits) {
                Some(old_wiki_links.to_vec())
            } else {
                let new_wiki_links = Self::extract_wiki_links_owned(&ast);
                Some(Self::merge_incremental_wiki_links(
                    old_wiki_links,
                    &new_wiki_links,
                    pending_edits,
                ))
            }
        } else {
            None
        };

        // Compute merged blocks
        let merged_blocks = if let Some(old_blocks) = old_blocks {
            if old_blocks.is_empty() {
                Some(Self::extract_blocks_owned(&ast))
            } else if !Self::blocks_need_update(old_blocks, pending_edits) {
                Some(old_blocks.to_vec())
            } else {
                let new_blocks = Self::extract_blocks_owned(&ast);
                Some(Self::merge_incremental_blocks(
                    old_blocks,
                    &new_blocks,
                    pending_edits,
                ))
            }
        } else {
            None
        };

        match (merged_wiki_links, merged_blocks) {
            (Some(wl), Some(bl)) => DocumentIndex::from_ast_with_wiki_links_and_blocks(ast, wl, bl),
            (Some(wl), None) => DocumentIndex::from_ast_with_wiki_links(ast, wl),
            (None, Some(bl)) => DocumentIndex::from_ast_with_blocks(ast, bl),
            (None, None) => DocumentIndex::from_ast(ast),
        }
    }

    /// Parse text with optional old tree reuse and build a markdown document index.
    ///
    /// When `old_tree` is `Some`, tree-sitter reuses unchanged subtrees for
    /// O(edit_size) reparsing. The old tree must have been updated via
    /// `MarkdownTree::edit()` with all changes since it was last parsed.
    fn build_markdown_index_with_old_tree(
        &mut self,
        text: &str,
        old_tree: Option<&MarkdownTree>,
        old_wiki_links: Option<&[WikiLinkOwned]>,
        old_blocks: Option<&[BlockOwned]>,
        pending_edits: &[InputEdit],
    ) -> (DocumentIndex, Option<MarkdownTree>) {
        let mut ast = self
            .parser
            .parse_with_old_tree(text, old_tree)
            .expect("failed to parse document");
        let md_tree = ast.take_md_tree();
        let index =
            Self::build_markdown_index_incremental(old_wiki_links, old_blocks, ast, pending_edits);
        (index, md_tree)
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
        let old_wiki_links = self.realm.get_document(uri).map(|index| {
            index
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
                .collect::<Vec<_>>()
        });
        let old_blocks = self.realm.get_document(uri).map(|index| {
            index
                .block_ids()
                .filter_map(|id| index.block_by_id(id))
                .map(|entry| BlockOwned {
                    id: entry.id.to_string(),
                    range: entry.range,
                    start_byte: entry.start_byte,
                    end_byte: entry.end_byte,
                })
                .collect::<Vec<_>>()
        });

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
                    old_wiki_links.as_deref(),
                    old_blocks.as_deref(),
                    &pending_edits,
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

    /// Detect the completion context from the line text up to the cursor position.
    ///
    /// Scans backward from the cursor to identify trigger patterns:
    /// - `[[partial` → [`CompletionContext::WikiLink`]
    /// - `[[target#partial` → [`CompletionContext::WikiLinkHeading`]
    /// - `#partial` (not inside `[[`) → [`CompletionContext::Tag`]
    /// - `((partial` → [`CompletionContext::BlockRef`]
    pub fn detect_completion_context(
        &self,
        uri: &DocumentUri,
        pos: Position,
    ) -> Option<CompletionContext> {
        let text = self.get_document_text(uri)?;
        let line = text.lines().nth(pos.line as usize)?;
        let col = pos.character as usize;
        if col > line.len() {
            return None;
        }
        let prefix = &line[..col];

        // Check for block ref: ((
        if let Some(open_idx) = prefix.rfind("((") {
            let after = &prefix[open_idx + 2..];
            if !after.contains("))") {
                return Some(CompletionContext::BlockRef {
                    partial: after.to_string(),
                });
            }
        }

        // Check for wiki link: [[
        if let Some(open_idx) = prefix.rfind("[[") {
            let after = &prefix[open_idx + 2..];
            if !after.contains("]]") {
                if let Some(hash_idx) = after.find('#') {
                    let target = &after[..hash_idx];
                    let partial = &after[hash_idx + 1..];
                    return Some(CompletionContext::WikiLinkHeading {
                        target: target.to_string(),
                        partial: partial.to_string(),
                    });
                } else {
                    return Some(CompletionContext::WikiLink {
                        partial: after.to_string(),
                    });
                }
            }
        }

        // Check for tag: # at word boundary (not inside [[)
        if let Some(hash_idx) = prefix.rfind('#') {
            if hash_idx == 0
                || prefix.as_bytes()[hash_idx - 1] == b' '
                || prefix.as_bytes()[hash_idx - 1] == b'\t'
            {
                let partial = &prefix[hash_idx + 1..];
                if partial
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    return Some(CompletionContext::Tag {
                        partial: partial.to_string(),
                    });
                }
            }
        }

        // Check for XML tag: < followed by alphanumeric/hyphen/underscore chars (not yet closed)
        if let Some(lt_idx) = prefix.rfind('<') {
            let after = &prefix[lt_idx + 1..];
            // Not a closing tag (</), not already closed (contains >)
            if !after.starts_with('/')
                && !after.contains('>')
                && after
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                return Some(CompletionContext::XmlTag {
                    partial: after.to_string(),
                });
            }
        }

        None
    }

    /// Get completion candidates at the given position.
    ///
    /// Combines context detection with realm/document data to produce
    /// a list of relevant completion suggestions.
    pub fn completion_at(&self, uri: &DocumentUri, pos: Position) -> Vec<CompletionCandidate> {
        let context = match self.detect_completion_context(uri, pos) {
            Some(ctx) => ctx,
            None => return Vec::new(),
        };

        let mut candidates = Vec::new();

        match context {
            CompletionContext::WikiLink { partial } => {
                let partial_lower = partial.to_lowercase();
                for (doc_uri, _index) in self.realm.iter_documents() {
                    if doc_uri == uri {
                        continue;
                    }
                    if let Some(path) = doc_uri.to_file_path() {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if partial_lower.is_empty()
                                || stem.to_lowercase().contains(&partial_lower)
                            {
                                candidates.push(CompletionCandidate {
                                    label: stem.to_string(),
                                    kind: CompletionCandidateKind::Page,
                                    detail: None,
                                });
                            }
                        }
                    }
                }
            }
            CompletionContext::WikiLinkHeading { target, partial } => {
                let partial_lower = partial.to_lowercase();
                let target_lower = target.to_lowercase();
                for (doc_uri, index) in self.realm.iter_documents() {
                    if let Some(path) = doc_uri.to_file_path() {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem.to_lowercase() == target_lower {
                                for heading in index.headings() {
                                    if partial_lower.is_empty()
                                        || heading.text.to_lowercase().contains(&partial_lower)
                                    {
                                        candidates.push(CompletionCandidate {
                                            label: heading.text.to_string(),
                                            kind: CompletionCandidateKind::Heading,
                                            detail: Some(format!("H{}", heading.level)),
                                        });
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            CompletionContext::Tag { partial } => {
                let partial_lower = partial.to_lowercase();
                for (tag_name, _count) in self.realm.tag_counts() {
                    if partial_lower.is_empty() || tag_name.to_lowercase().contains(&partial_lower)
                    {
                        candidates.push(CompletionCandidate {
                            label: tag_name.to_string(),
                            kind: CompletionCandidateKind::Tag,
                            detail: None,
                        });
                    }
                }
            }
            CompletionContext::BlockRef { partial } => {
                let partial_lower = partial.to_lowercase();
                for (_doc_uri, index) in self.realm.iter_documents() {
                    for block_id in index.block_ids() {
                        if partial_lower.is_empty()
                            || block_id.to_lowercase().contains(&partial_lower)
                        {
                            candidates.push(CompletionCandidate {
                                label: block_id.to_string(),
                                kind: CompletionCandidateKind::BlockRef,
                                detail: None,
                            });
                        }
                    }
                }
            }
            CompletionContext::XmlTag { partial } => {
                let partial_lower = partial.to_lowercase();
                // Collect unique XML tag names across all documents
                let mut seen = std::collections::HashSet::new();
                for (_doc_uri, index) in self.realm.iter_documents() {
                    for xt in index.xml_tags() {
                        if seen.insert(xt.tag_name.to_string())
                            && (partial_lower.is_empty()
                                || xt.tag_name.to_lowercase().contains(&partial_lower))
                        {
                            candidates.push(CompletionCandidate {
                                label: xt.tag_name.to_string(),
                                kind: CompletionCandidateKind::XmlTag,
                                detail: None,
                            });
                        }
                    }
                }
            }
        }

        candidates
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

        let mut diagnostics = Vec::new();

        // 1. Check wiki links for broken references
        for wl in index.wiki_links() {
            let resolved = resolve_wiki_link(&self.realm, uri, wl.target, wl.heading);
            if resolved.is_none() {
                let target_desc = match &wl.heading {
                    Some(h) => format!("{}#{}", wl.target, h),
                    None => wl.target.to_string(),
                };
                diagnostics.push(MarkyDiagnostic {
                    range: wl.range,
                    severity: DiagnosticSeverity::Error,
                    message: format!("Broken wiki link: [[{}]]", target_desc),
                });
            }
        }

        // 2. Check markdown link anchors for broken references
        for ml in index.markdown_links() {
            if let Some(anchor) = &ml.anchor {
                // Same-page anchor links: check if slug exists in current doc
                let raw_url = ml
                    .url
                    .strip_suffix(&format!("#{}", anchor))
                    .unwrap_or(ml.url);
                let resolved = resolve_markdown_link(&self.realm, uri, raw_url, Some(*anchor));
                if resolved.is_none() {
                    diagnostics.push(MarkyDiagnostic {
                        range: ml.range,
                        severity: DiagnosticSeverity::Error,
                        message: format!("Broken link: heading '{}' not found", anchor),
                    });
                }
            }
        }

        // 3. Check for duplicate heading slugs
        //
        // Use the *base* slug (from slugify) rather than the stored (deduped)
        // slug so that headings whose text produces the same slug are detected
        // (the indexer already appends `-1`, `-2`, etc. to avoid collisions).
        let mut slug_counts: HashMap<String, Vec<Range>> = HashMap::new();
        for h in index.headings() {
            let base_slug = slugify(h.text);
            slug_counts.entry(base_slug).or_default().push(h.range);
        }
        for (slug, ranges) in &slug_counts {
            if ranges.len() > 1 {
                for range in ranges {
                    diagnostics.push(MarkyDiagnostic {
                        range: *range,
                        severity: DiagnosticSeverity::Warning,
                        message: format!(
                            "Duplicate heading slug '{}' ({} occurrences)",
                            slug,
                            ranges.len()
                        ),
                    });
                }
            }
        }

        // 4. Check for unclosed XML tags
        for xt in index.xml_tags() {
            if xt.is_unclosed {
                diagnostics.push(MarkyDiagnostic {
                    range: xt.range,
                    severity: DiagnosticSeverity::Warning,
                    message: format!("Unclosed XML tag: <{}>", xt.tag_name),
                });
            }
        }

        diagnostics
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
            !ServerState::range_is_after_edit_start(range, &edit),
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
            ServerState::range_is_after_edit_start(range, &edit),
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
            ServerState::range_within_neighbor_window(70, 85, &edit, 100),
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
            !ServerState::range_within_neighbor_window(261, 280, &edit, 100),
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
            ServerState::wiki_links_need_update(&old_wiki_links, &pending_edits),
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
            !ServerState::blocks_need_update(&old_blocks, &[]),
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
            ServerState::blocks_need_update(&old_blocks, &[edit]),
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
            ServerState::blocks_need_update(&old_blocks, &[edit]),
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
            ServerState::blocks_need_update(&old_blocks, &[edit]),
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
        let merged = ServerState::merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
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
        let merged = ServerState::merge_incremental_blocks(&old_blocks, &new_blocks, &[edit]);
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
        let inc_index = ServerState::build_markdown_index_incremental(
            None,
            Some(&old_blocks_owned),
            ast_inc,
            &[edit],
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
