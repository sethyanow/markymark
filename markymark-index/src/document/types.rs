//! Document index entry types.

use hashbrown::HashMap;
use markymark_core::prelude::*;

/// A heading entry in the document index.
#[derive(Debug, Clone)]
pub struct HeadingEntry<'arena> {
    /// The heading text.
    pub text: &'arena str,
    /// URL-safe slug derived from the heading text.
    pub slug: &'arena str,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range of the heading.
    pub range: Range,
}

/// A block entry in the document index (Obsidian `^block-id`).
#[derive(Debug, Clone)]
pub struct BlockEntry<'arena> {
    /// The block identifier.
    pub id: &'arena str,
    /// Source range of the block.
    pub range: Range,
}

/// A table-of-contents entry.
#[derive(Debug, Clone)]
pub struct TocEntry<'arena> {
    /// Heading text.
    pub text: &'arena str,
    /// URL-safe slug.
    pub slug: &'arena str,
    /// Heading level (1-6).
    pub level: u8,
    /// Nesting depth relative to the root (0-based).
    pub depth: usize,
}

/// A node in the document outline tree.
#[derive(Debug, Clone)]
pub struct OutlineNode<'arena> {
    /// The heading at this node, if any (root node has `None`).
    pub heading: Option<HeadingEntry<'arena>>,
    /// Child outline nodes.
    pub children: &'arena [OutlineNode<'arena>],
}

/// A wiki link entry stored in the index.
#[derive(Debug, Clone)]
pub struct WikiLinkEntry<'arena> {
    /// Target page name.
    pub target: &'arena str,
    /// Optional alias text.
    pub alias: Option<&'arena str>,
    /// Optional heading anchor within the target.
    pub heading: Option<&'arena str>,
    /// Source range.
    pub range: Range,
    /// Byte offset of the opening `[[`.
    pub start_byte: usize,
    /// Byte offset one past the closing `]]`.
    pub end_byte: usize,
}

/// Owned wiki-link payload used by incremental merge paths before arena allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLinkOwned {
    /// Target page name.
    pub target: String,
    /// Optional alias text.
    pub alias: Option<String>,
    /// Optional heading anchor within the target.
    pub heading: Option<String>,
    /// Source range.
    pub range: Range,
    /// Byte offset of the opening `[[`.
    pub start_byte: usize,
    /// Byte offset one past the closing `]]`.
    pub end_byte: usize,
}

/// A tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct TagEntry<'arena> {
    /// Tag name (without leading `#`).
    pub name: &'arena str,
}

/// A markdown link entry stored in the index.
#[derive(Debug, Clone)]
pub struct MarkdownLinkEntry<'arena> {
    /// Link display text.
    pub text: &'arena str,
    /// Link URL.
    pub url: &'arena str,
    /// Optional anchor/fragment.
    pub anchor: Option<&'arena str>,
    /// Source range.
    pub range: Range,
}

/// An XML tag entry stored in the index.
///
/// Uses standard `HashMap` (not `ArenaHashMap`) for attributes because
/// `Bump: !Sync` makes `&Bump: !Send`, which would prevent `DocumentIndex`
/// from satisfying `Send + 'static` required by tower-lsp. Keys and values
/// still borrow from the arena; only the map's internal buckets are heap-allocated.
#[derive(Debug, Clone)]
pub struct XmlTagEntry<'arena> {
    /// Tag name (e.g. "agent", "goal", "task").
    pub tag_name: &'arena str,
    /// Tag attributes as key-value pairs. Standard allocator for Send safety;
    /// keys/values borrow from arena.
    pub attributes: HashMap<&'arena str, &'arena str>,
    /// Whether this is a self-closing tag (e.g. `<br/>`).
    pub is_self_closing: bool,
    /// Whether this tag has no matching closing tag.
    pub is_unclosed: bool,
    /// Source range of the entire tag.
    pub range: Range,
}
