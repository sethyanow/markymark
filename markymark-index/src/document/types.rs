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

/// Owned tag payload used by scan-to-index construction before arena allocation.
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
    /// Whether this tag was found inline (within a paragraph) rather than block-level.
    pub is_inline: bool,
    /// Source range (line/col).
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Kind of symbol referenced by an inline code span.
///
/// Tier 1 (backtick extraction) always sets `None` — the kind cannot be
/// determined from syntax alone. Tier 2+ may infer kind from context
/// (e.g. `DocumentArena` following "struct" in prose).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// A struct type.
    Struct,
    /// A trait.
    Trait,
    /// A function or method.
    Function,
    /// A type alias or other named type.
    Type,
    /// A constant or static.
    Constant,
    /// A module or crate.
    Module,
}

/// An inline code span entry stored in the index.
#[derive(Debug, Clone)]
pub struct CodeSpanEntry<'arena> {
    /// The backtick-delimited text content (decoded).
    pub text: &'arena str,
    /// Source range of the code span.
    pub range: Range,
    /// Byte offset of the opening backtick.
    pub start_byte: usize,
    /// Byte offset one past the closing backtick.
    pub end_byte: usize,
    /// Language hint (None for Tier 1 — all backtick spans are untyped).
    pub language_hint: Option<&'arena str>,
    /// Symbol kind (None for Tier 1 — cannot determine struct/fn/trait from backtick alone).
    pub kind: Option<SymbolKind>,
}

/// Owned code span payload used by incremental merge paths before arena allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpanOwned {
    /// The backtick-delimited text content (decoded).
    pub text: String,
    /// Source range of the code span.
    pub range: Range,
    /// Byte offset of the opening backtick.
    pub start_byte: usize,
    /// Byte offset one past the closing backtick.
    pub end_byte: usize,
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

/// A frontmatter value stored in the index (arena-allocated).
#[derive(Debug, Clone)]
pub enum FrontmatterValueEntry<'arena> {
    /// A simple string value.
    String(&'arena str),
    /// An integer value.
    Integer(i64),
    /// A floating-point value (always finite).
    Float(f64),
    /// A boolean value.
    Boolean(bool),
    /// A list of typed values.
    List(&'arena [FrontmatterValueEntry<'arena>]),
    /// A map of key-value pairs.
    Map(&'arena [(&'arena str, FrontmatterValueEntry<'arena>)]),
    /// An explicit null value.
    Null,
}

/// A frontmatter key-value entry stored in the index.
#[derive(Debug, Clone)]
pub struct FrontmatterEntry<'arena> {
    /// The key.
    pub key: &'arena str,
    /// The value.
    pub value: FrontmatterValueEntry<'arena>,
}

/// An owned frontmatter value for cross-module transfer (not arena-allocated).
#[derive(Debug, Clone)]
pub enum FrontmatterValueOwned {
    /// A simple string value.
    String(String),
    /// An integer value.
    Integer(i64),
    /// A floating-point value (always finite).
    Float(f64),
    /// A boolean value.
    Boolean(bool),
    /// A list of typed values.
    List(Vec<FrontmatterValueOwned>),
    /// A map of key-value pairs.
    Map(Vec<(String, FrontmatterValueOwned)>),
    /// An explicit null value.
    Null,
}

/// An owned frontmatter key-value entry for cross-module transfer (not arena-allocated).
#[derive(Debug, Clone)]
pub struct FrontmatterOwnedEntry {
    /// The key.
    pub key: String,
    /// The value.
    pub value: FrontmatterValueOwned,
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
    /// Whether this tag was found inline (within a paragraph) rather than block-level.
    pub is_inline: bool,
    /// Source range of the entire tag.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// An embed entry (`![[target]]`) stored in the index.
#[derive(Debug, Clone)]
pub struct EmbedEntry<'arena> {
    /// The embedded resource path.
    pub target: &'arena str,
    /// Source range.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned embed payload for incremental merge paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedOwned {
    /// The embedded resource path.
    pub target: String,
    /// Source range.
    pub range: Range,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
}

/// A task entry (checkbox item) stored in the index.
#[derive(Debug, Clone)]
pub struct TaskEntry<'arena> {
    /// Checkbox state (e.g. "unchecked", "checked", "in_progress").
    pub state: &'arena str,
    /// Task description text.
    pub text: &'arena str,
    /// Source range.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned task payload for incremental merge paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOwned {
    /// Checkbox state.
    pub state: String,
    /// Task description text.
    pub text: String,
    /// Source range.
    pub range: Range,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
}

/// A callout entry (Obsidian `[!type]` blockquote) stored in the index.
#[derive(Debug, Clone)]
pub struct CalloutEntry<'arena> {
    /// Callout type (e.g. "note", "warning", "tip").
    pub callout_type: &'arena str,
    /// Optional callout title.
    pub title: Option<&'arena str>,
    /// Source range.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned callout payload for incremental merge paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalloutOwned {
    /// Callout type.
    pub callout_type: String,
    /// Optional callout title.
    pub title: Option<String>,
    /// Source range.
    pub range: Range,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
}

/// A query block entry (Logseq `{{query ...}}`) stored in the index.
#[derive(Debug, Clone)]
pub struct QueryBlockEntry<'arena> {
    /// The query text.
    pub query: &'arena str,
    /// Source range.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned query block payload for incremental merge paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryBlockOwned {
    /// The query text.
    pub query: String,
    /// Source range.
    pub range: Range,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
}

/// A link definition entry (`[label]: url "title"`) stored in the index.
#[derive(Debug, Clone)]
pub struct LinkDefinitionEntry<'arena> {
    /// The link label.
    pub label: &'arena str,
    /// The link URL.
    pub url: &'arena str,
    /// Optional title.
    pub title: Option<&'arena str>,
    /// Source range.
    pub range: Range,
    /// Start byte offset in the source document.
    pub start_byte: usize,
    /// End byte offset in the source document.
    pub end_byte: usize,
}

/// Owned link definition payload for incremental merge paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDefinitionOwned {
    /// The link label.
    pub label: String,
    /// The link URL.
    pub url: String,
    /// Optional title.
    pub title: Option<String>,
    /// Source range.
    pub range: Range,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
}
