//! Server state: document store, parsing, and indexing.

use std::collections::HashMap;

use markymark_core::structured::{DocumentKind, KeyEntry, ValueKind};
use markymark_core::{DocumentUri, Position, Range};
use markymark_index::resolution::{resolve_markdown_link, resolve_wiki_link};
use markymark_index::{
    slugify, AnyDocumentIndex, DocumentIndex, HeadingEntry, MarkdownLinkEntry, RealmIndex,
    StructuredDocumentIndex, WikiLinkEntry, XmlTagEntry,
};
use markymark_parser::structured::parse_structured;
use markymark_parser::Parser;

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
pub enum SymbolAtPosition {
    /// A heading line.
    Heading(HeadingEntry<'static>),
    /// A wiki link.
    WikiLink(WikiLinkEntry<'static>),
    /// A markdown link.
    MarkdownLink(MarkdownLinkEntry<'static>),
    /// An XML tag.
    XmlTag(XmlTagEntry<'static>),
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

/// The internal state of the LSP server.
///
/// Manages document text storage, parsed ASTs, and the realm index.
/// The parser is stored here to avoid re-creating it on every parse call.
pub struct ServerState {
    /// Raw document text keyed by URI string.
    documents: HashMap<String, String>,
    /// The realm index for cross-document lookups.
    realm: RealmIndex,
    /// Reusable markdown parser instance.
    parser: Parser,
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
        }
    }

    /// Parse text and build a markdown document index.
    fn build_markdown_index(&mut self, text: &str) -> DocumentIndex {
        let ast = self.parser.parse(text).expect("failed to parse document");
        DocumentIndex::from_ast(ast)
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
                let index = self.build_markdown_index(&text);
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
        self.realm.remove_document(uri);
        let kind = Self::document_kind_from_uri(uri);
        self.documents
            .insert(uri.as_str().to_string(), text.clone());

        match kind {
            Some(DocumentKind::Markdown) | None => {
                let index = self.build_markdown_index(&text);
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

    /// Handle a document being closed: remove from store and index.
    pub fn close_document(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri.as_str());
        self.realm.remove_document(uri);
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
    pub fn symbol_at_position(&self, uri: &DocumentUri, pos: Position) -> Option<SymbolAtPosition> {
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
