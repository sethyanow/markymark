//! Symbol-at-position detection for cursor navigation.

use markymark_core::structured::{DocumentKind, KeyEntry, ValueKind};
use markymark_core::{DocumentUri, Position};
use markymark_index::{CodeSpanEntry, HeadingEntry, MarkdownLinkEntry, WikiLinkEntry, XmlTagEntry};

use super::ServerState;

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
    /// An inline code span (backtick-delimited text).
    CodeSpan(CodeSpanEntry<'a>),
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

impl ServerState {
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

        // Check code spans
        for cs in index.code_spans() {
            if cs.range.contains(pos) {
                return Some(SymbolAtPosition::CodeSpan(cs.clone()));
            }
        }

        None
    }
}
