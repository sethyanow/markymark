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
    /// Byte offset of the `^` character.
    pub start_byte: usize,
    /// Byte offset one past the last character of the block ID.
    pub end_byte: usize,
}

/// Owned block payload used by incremental merge paths before arena allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockOwned {
    /// The block identifier.
    pub id: String,
    /// Source range of the block.
    pub range: Range,
    /// Byte offset of the `^` character.
    pub start_byte: usize,
    /// Byte offset one past the last character of the block ID.
    pub end_byte: usize,
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

/// Owned tag payload used by incremental merge paths before arena allocation.
///
/// Note: `Tag` in the parser has no source range, so tags cannot be incrementally
/// merged. Always pass `None` for `IncrementalOverrides::tags`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagOwned {
    /// Tag name (without leading `#`).
    pub name: String,
}

/// Owned markdown link payload used by incremental merge paths before arena allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLinkOwned {
    /// Link display text.
    pub text: String,
    /// Link URL.
    pub url: String,
    /// Optional anchor/fragment.
    pub anchor: Option<String>,
    /// Source range (line/col).
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned XML tag payload used by incremental merge paths before arena allocation.
///
/// Attributes are stored sorted by key for deterministic ordering, preventing
/// false positives in equality comparisons regardless of extraction order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlTagOwned {
    /// Tag name.
    pub tag_name: String,
    /// Attributes as key-value pairs, sorted by key.
    pub attributes: Vec<(String, String)>,
    /// Whether this is a self-closing tag.
    pub is_self_closing: bool,
    /// Whether this tag has no matching closing tag.
    pub is_unclosed: bool,
    /// Source range (line/col).
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Overrides for each independent extractor used by the incremental index path.
///
/// `None` means: extract fresh from the AST (no reuse).
/// `Some(vec)` means: use the provided owned data instead of re-extracting.
///
/// `tags` is always `None` because [`Tag`][markymark_parser] has no source range
/// and cannot be incrementally merged. It is included for API completeness only.
#[derive(Debug, Default)]
pub struct IncrementalOverrides {
    /// Merged wiki-links from the incremental path, or `None` to re-extract.
    pub wiki_links: Option<Vec<WikiLinkOwned>>,
    /// Merged block IDs from the incremental path, or `None` to re-extract.
    pub blocks: Option<Vec<BlockOwned>>,
    /// Always `None` — tags have no range, cannot be incrementally merged.
    pub tags: Option<Vec<TagOwned>>,
    /// Merged markdown links from the incremental path, or `None` to re-extract.
    pub markdown_links: Option<Vec<MarkdownLinkOwned>>,
    /// Merged XML tags from the incremental path, or `None` to re-extract.
    pub xml_tags: Option<Vec<XmlTagOwned>>,
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
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// A frontmatter value stored in the index.
#[derive(Debug, Clone)]
pub enum FrontmatterValueEntry<'arena> {
    /// A simple string value.
    String(&'arena str),
    /// A list of string values.
    List(&'arena [&'arena str]),
}

/// A frontmatter key-value entry stored in the index.
#[derive(Debug, Clone)]
pub struct FrontmatterEntry<'arena> {
    /// The key.
    pub key: &'arena str,
    /// The value.
    pub value: FrontmatterValueEntry<'arena>,
}

/// A Logseq property value stored in the index.
#[derive(Debug, Clone)]
pub enum PropertyValueEntry<'arena> {
    /// A simple string value.
    String(&'arena str),
    /// A list of string values.
    List(&'arena [&'arena str]),
    /// A Logseq page reference.
    PageRef(&'arena str),
}

/// A Logseq property key-value entry stored in the index.
#[derive(Debug, Clone)]
pub struct PropertyEntry<'arena> {
    /// The key.
    pub key: &'arena str,
    /// The value.
    pub value: PropertyValueEntry<'arena>,
}

/// A Logseq block reference entry — an outgoing `((uuid))` link.
#[derive(Debug, Clone, Copy)]
pub struct BlockRefEntry<'arena> {
    /// The UUID referenced by `((uuid))`.
    pub uuid: &'arena str,
    /// Source range of the full `((uuid))` pattern.
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
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}
