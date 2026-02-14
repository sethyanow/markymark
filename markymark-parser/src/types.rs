//! Parser types for arena-allocated markdown AST.
//!
//! All types use `'arena` lifetime for borrowed data, enabling efficient
//! bulk deallocation via bumpalo arenas.

use markymark_core::prelude::*;
use std::collections::HashMap;
use tree_sitter::Node;

/// Allocate a string in the arena and return it as `&'arena str`.
/// This helper is needed because `Bump::alloc_str` returns `&mut str`,
/// which doesn't automatically coerce in all contexts.
#[inline]
fn arena_alloc_str<'a>(arena: &'a bumpalo::Bump, s: &str) -> &'a str {
    let allocated: &mut str = arena.alloc_str(s);
    allocated
}

/// An element in the markdown AST
#[derive(Debug, Clone)]
pub enum Element<'arena> {
    /// Heading element
    Heading(Heading<'arena>),
    /// Paragraph element
    Paragraph(Paragraph<'arena>),
    /// List item
    ListItem(ListItem<'arena>),
    /// Other elements (placeholder for now)
    Other,
}

impl<'arena> Element<'arena> {
    pub(crate) fn from_node(
        node: Node,
        source: &str,
        arena: &'arena bumpalo::Bump,
    ) -> CoreResult<Option<Self>> {
        match node.kind() {
            "atx_heading" | "setext_heading" => {
                Ok(Some(Element::Heading(Heading::from_node(node, source, arena)?)))
            }
            "paragraph" => Ok(Some(Element::Paragraph(Paragraph::from_node(
                node, source, arena,
            )?))),
            "list_item" => Ok(Some(Element::ListItem(ListItem::from_node(
                node, source, arena,
            )?))),
            _ => Ok(None), // Skip unknown node types for now
        }
    }

    /// Try to get as heading
    pub fn as_heading(&self) -> Option<&Heading<'arena>> {
        match self {
            Element::Heading(h) => Some(h),
            _ => None,
        }
    }

    /// Try to get as paragraph
    pub fn as_paragraph(&self) -> Option<&Paragraph<'arena>> {
        match self {
            Element::Paragraph(p) => Some(p),
            _ => None,
        }
    }
}

/// A heading
#[derive(Debug, Clone)]
pub struct Heading<'arena> {
    level: u8,
    text: &'arena str,
    range: Range,
}

impl<'arena> Heading<'arena> {
    fn from_node(node: Node, source: &str, arena: &'arena bumpalo::Bump) -> CoreResult<Self> {
        let level = match node.kind() {
            "atx_heading" => {
                // Count # symbols
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                text.chars().take_while(|&c| c == '#').count() as u8
            }
            "setext_heading" => {
                // Setext headings are level 1 or 2
                1 // Simplified for now
            }
            _ => 1,
        };

        // Extract heading text content
        let mut text = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "atx_h1_marker"
                || child.kind() == "atx_h2_marker"
                || child.kind() == "atx_h3_marker"
                || child.kind() == "atx_h4_marker"
                || child.kind() == "atx_h5_marker"
                || child.kind() == "atx_h6_marker"
            {
                continue; // Skip markers
            }
            if let Ok(child_text) = child.utf8_text(source.as_bytes()) {
                text.push_str(child_text.trim());
            }
        }

        let range = Range::new(
            Position::new(
                node.start_position().row as u32,
                node.start_position().column as u32,
            ),
            Position::new(
                node.end_position().row as u32,
                node.end_position().column as u32,
            ),
        );

        // Allocate text in arena
        let text = arena_alloc_str(arena, &text);

        Ok(Self { level, text, range })
    }

    /// Get heading level (1-6)
    pub fn level(&self) -> u8 {
        self.level
    }

    /// Get heading text
    pub fn text(&self) -> &'arena str {
        self.text
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }

    /// Create a heading directly (used for Logseq-style headings in list items).
    pub(crate) fn new(level: u8, text: &'arena str, range: Range) -> Self {
        Self { level, text, range }
    }
}

/// A paragraph
#[derive(Debug, Clone)]
pub struct Paragraph<'arena> {
    text: &'arena str,
    #[allow(dead_code)]
    range: Range,
}

impl<'arena> Paragraph<'arena> {
    fn from_node(node: Node, source: &str, arena: &'arena bumpalo::Bump) -> CoreResult<Self> {
        let text = node
            .utf8_text(source.as_bytes())
            .map_err(|e| CoreError::Message(format!("UTF-8 error: {}", e)))?
            .trim();

        let range = Range::new(
            Position::new(
                node.start_position().row as u32,
                node.start_position().column as u32,
            ),
            Position::new(
                node.end_position().row as u32,
                node.end_position().column as u32,
            ),
        );

        // Allocate text in arena
        let text = arena_alloc_str(arena, text);

        Ok(Self { text, range })
    }

    /// Get paragraph text
    pub fn text(&self) -> &'arena str {
        self.text
    }
}

/// A list item
#[derive(Debug, Clone)]
pub struct ListItem<'arena> {
    #[allow(dead_code)]
    text: &'arena str,
    #[allow(dead_code)]
    properties_map: HashMap<&'arena str, &'arena str>,
    #[allow(dead_code)]
    children_list: &'arena [ListItem<'arena>],
}

impl<'arena> ListItem<'arena> {
    pub(crate) fn from_node(
        node: Node,
        source: &str,
        arena: &'arena bumpalo::Bump,
    ) -> CoreResult<Self> {
        let text = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .trim();

        let mut properties_map = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once("::") else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();

            if key.is_empty() || value.is_empty() {
                continue;
            }

            // Allocate key and value in arena
            properties_map.insert(arena_alloc_str(arena, key), arena_alloc_str(arena, value));
        }

        Ok(Self {
            text: arena_alloc_str(arena, text),
            properties_map,
            children_list: &[],
        })
    }

    pub(crate) fn list_items_from_list_node(
        node: Node,
        source: &str,
        arena: &'arena bumpalo::Bump,
    ) -> CoreResult<&'arena [ListItem<'arena>]> {
        let mut items = bumpalo::collections::Vec::new_in(arena);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "list_item" {
                let item = Self::from_node(child, source, arena)?;

                // Note: children_list population would require recursive arena allocation
                // For now, we leave it empty - this can be enhanced later

                items.push(item);
            }
        }

        Ok(items.into_bump_slice())
    }

    fn find_first_list_descendant(node: Node) -> Option<Node> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "tight_list" || child.kind() == "loose_list" {
                return Some(child);
            }

            if let Some(found) = Self::find_first_list_descendant(child) {
                return Some(found);
            }
        }

        None
    }

    /// Get list item properties
    pub fn properties(&self) -> &HashMap<&'arena str, &'arena str> {
        &self.properties_map
    }

    /// Get child list items
    pub fn children(&self) -> Option<&'arena [ListItem<'arena>]> {
        if self.children_list.is_empty() {
            None
        } else {
            Some(self.children_list)
        }
    }
}

// Extraction types (created via `new()` functions, used by extract module)

/// A wiki link
#[derive(Debug, Clone)]
pub struct WikiLink<'arena> {
    target: &'arena str,
    alias: Option<&'arena str>,
    heading: Option<&'arena str>,
    block_id: Option<&'arena str>,
    range: Range,
}

impl<'arena> WikiLink<'arena> {
    /// Create a new wiki link
    pub(crate) fn new(
        target: &'arena str,
        alias: Option<&'arena str>,
        heading: Option<&'arena str>,
        block_id: Option<&'arena str>,
        range: Range,
    ) -> Self {
        Self {
            target,
            alias,
            heading,
            block_id,
            range,
        }
    }

    /// Get target page
    pub fn target_page(&self) -> Option<&'arena str> {
        if self.target.is_empty() {
            None
        } else {
            Some(self.target)
        }
    }

    /// Get alias
    pub fn alias(&self) -> Option<&'arena str> {
        self.alias
    }

    /// Get target heading
    pub fn target_heading(&self) -> Option<&'arena str> {
        self.heading
    }

    /// Get target block ID
    pub fn target_block_id(&self) -> Option<&'arena str> {
        self.block_id
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A markdown link
#[derive(Debug, Clone)]
pub struct MarkdownLink<'arena> {
    text: &'arena str,
    url: &'arena str,
    anchor: Option<&'arena str>,
    reference: Option<&'arena str>,
    range: Range,
}

impl<'arena> MarkdownLink<'arena> {
    /// Create a new markdown link
    pub(crate) fn new(
        text: &'arena str,
        url: &'arena str,
        anchor: Option<&'arena str>,
        reference: Option<&'arena str>,
        range: Range,
    ) -> Self {
        Self {
            text,
            url,
            anchor,
            reference,
            range,
        }
    }

    /// Get link text
    pub fn text(&self) -> &'arena str {
        self.text
    }

    /// Get URL
    pub fn url(&self) -> &'arena str {
        self.url
    }

    /// Get anchor
    pub fn anchor(&self) -> Option<&'arena str> {
        self.anchor
    }

    /// Get reference
    pub fn reference(&self) -> Option<&'arena str> {
        self.reference
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A link definition
#[derive(Debug, Clone)]
pub struct LinkDefinition<'arena> {
    label: &'arena str,
    url: &'arena str,
    title: Option<&'arena str>,
}

impl<'arena> LinkDefinition<'arena> {
    /// Create a new link definition
    pub(crate) fn new(label: &'arena str, url: &'arena str, title: Option<&'arena str>) -> Self {
        Self { label, url, title }
    }

    /// Get label
    pub fn label(&self) -> &'arena str {
        self.label
    }

    /// Get URL
    pub fn url(&self) -> &'arena str {
        self.url
    }

    /// Get title
    pub fn title(&self) -> Option<&'arena str> {
        self.title
    }
}

/// Block ID (Obsidian)
#[derive(Debug, Clone)]
pub struct BlockId<'arena> {
    id: &'arena str,
}

impl<'arena> BlockId<'arena> {
    /// Create a new block ID
    pub(crate) fn new(id: &'arena str) -> Self {
        Self { id }
    }

    /// Get ID
    pub fn id(&self) -> &'arena str {
        self.id
    }
}

/// Block reference (Logseq)
#[derive(Debug, Clone)]
pub struct BlockRef<'arena> {
    uuid: &'arena str,
}

impl<'arena> BlockRef<'arena> {
    /// Create a new block reference
    pub(crate) fn new(uuid: &'arena str) -> Self {
        Self { uuid }
    }

    /// Get UUID
    pub fn uuid(&self) -> &'arena str {
        self.uuid
    }
}

/// A tag
#[derive(Debug, Clone)]
pub struct Tag<'arena> {
    name: &'arena str,
}

impl<'arena> Tag<'arena> {
    /// Create a new tag
    pub(crate) fn new(name: &'arena str) -> Self {
        Self { name }
    }

    /// Get tag name
    pub fn name(&self) -> &'arena str {
        self.name
    }

    /// Get tag segments (for nested tags like #project/feature/bug)
    pub fn segments(&self) -> Vec<&'arena str> {
        self.name.split('/').collect()
    }
}

/// An embed
#[derive(Debug, Clone)]
pub struct Embed<'arena> {
    target: &'arena str,
}

impl<'arena> Embed<'arena> {
    /// Create a new embed
    pub(crate) fn new(target: &'arena str) -> Self {
        Self { target }
    }

    /// Get target
    pub fn target(&self) -> &'arena str {
        self.target
    }

    /// Check if this is an embed
    pub fn is_embed(&self) -> bool {
        true
    }
}

/// A task
#[derive(Debug, Clone)]
pub struct Task<'arena> {
    state: TaskState<'arena>,
}

impl<'arena> Task<'arena> {
    /// Create a new task
    pub(crate) fn new(state: TaskState<'arena>) -> Self {
        Self { state }
    }

    /// Get task state
    pub fn state(&self) -> &TaskState<'arena> {
        &self.state
    }
}

/// Task state
#[derive(Debug, Clone)]
pub struct TaskState<'arena> {
    name: &'arena str,
}

impl<'arena> TaskState<'arena> {
    /// Create a new task state
    pub(crate) fn new(name: &'arena str) -> Self {
        Self { name }
    }

    /// Get state as string
    pub fn as_str(&self) -> &'arena str {
        self.name
    }
}

/// Callout (Obsidian)
#[derive(Debug, Clone)]
pub struct Callout<'arena> {
    callout_type: &'arena str,
    title: Option<&'arena str>,
}

impl<'arena> Callout<'arena> {
    /// Create a new callout
    pub(crate) fn new(callout_type: &'arena str, title: Option<&'arena str>) -> Self {
        Self {
            callout_type,
            title,
        }
    }

    /// Get callout type
    pub fn callout_type(&self) -> &'arena str {
        self.callout_type
    }

    /// Get title
    pub fn title(&self) -> Option<&'arena str> {
        self.title
    }
}

/// Query block (Logseq)
#[derive(Debug, Clone)]
pub struct QueryBlock<'arena> {
    query: &'arena str,
}

impl<'arena> QueryBlock<'arena> {
    /// Create a new query block
    pub(crate) fn new(query: &'arena str) -> Self {
        Self { query }
    }

    /// Get query text
    pub fn query_text(&self) -> &'arena str {
        self.query
    }
}

/// Frontmatter
#[derive(Debug, Clone)]
pub struct Frontmatter<'arena> {
    data: HashMap<&'arena str, FrontmatterValue<'arena>>,
}

impl<'arena> Frontmatter<'arena> {
    /// Create new frontmatter
    pub(crate) fn new(data: HashMap<&'arena str, FrontmatterValue<'arena>>) -> Self {
        Self { data }
    }

    /// Get string value
    pub fn get_string(&self, key: &str) -> Option<&'arena str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    /// Get list value
    pub fn get_list(&self, key: &str) -> Option<Vec<&'arena str>> {
        self.data.get(key).and_then(|v| v.as_list())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FrontmatterValue<'arena> {
    String(&'arena str),
    List(&'arena [&'arena str]),
}

impl<'arena> FrontmatterValue<'arena> {
    fn as_string(&self) -> Option<&'arena str> {
        match self {
            FrontmatterValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<Vec<&'arena str>> {
        match self {
            FrontmatterValue::List(list) => Some(list.iter().copied().collect()),
            _ => None,
        }
    }
}

/// Properties (Logseq)
#[derive(Debug, Clone)]
pub struct Properties<'arena> {
    data: HashMap<&'arena str, PropertyValue<'arena>>,
}

impl<'arena> Properties<'arena> {
    /// Create new properties
    pub(crate) fn new(data: HashMap<&'arena str, PropertyValue<'arena>>) -> Self {
        Self { data }
    }

    /// Get property
    pub fn get(&self, key: &str) -> Option<&PropertyValue<'arena>> {
        self.data.get(key)
    }
}

/// A property value (Logseq)
#[derive(Debug, Clone)]
pub enum PropertyValue<'arena> {
    /// String value
    String(&'arena str),
    /// List of values
    List(&'arena [&'arena str]),
    /// Page reference
    PageRef(&'arena str),
}

impl<'arena> PropertyValue<'arena> {
    /// Get as string
    pub fn as_str(&self) -> &'arena str {
        match self {
            PropertyValue::String(s) => s,
            PropertyValue::PageRef(s) => s,
            _ => "",
        }
    }

    /// Check if this is a list
    pub fn is_list(&self) -> bool {
        matches!(self, PropertyValue::List(_))
    }

    /// Check if this is a page reference
    pub fn is_page_ref(&self) -> bool {
        matches!(self, PropertyValue::PageRef(_))
    }
}

/// An XML/HTML tag element extracted from markdown
#[derive(Debug, Clone)]
pub struct XmlTag<'arena> {
    tag_name: &'arena str,
    attributes: HashMap<&'arena str, &'arena str>,
    is_self_closing: bool,
    is_unclosed: bool,
    content: Option<&'arena str>,
    range: Range,
}

impl<'arena> XmlTag<'arena> {
    /// Create a new XML tag
    pub(crate) fn new(
        tag_name: &'arena str,
        attributes: HashMap<&'arena str, &'arena str>,
        is_self_closing: bool,
        content: Option<&'arena str>,
        range: Range,
    ) -> Self {
        Self {
            tag_name,
            attributes,
            is_self_closing,
            is_unclosed: false,
            content,
            range,
        }
    }

    /// Create an unclosed XML tag (opening tag with no matching close)
    pub(crate) fn unclosed(
        tag_name: &'arena str,
        attributes: HashMap<&'arena str, &'arena str>,
        range: Range,
    ) -> Self {
        Self {
            tag_name,
            attributes,
            is_self_closing: false,
            is_unclosed: true,
            content: None,
            range,
        }
    }

    /// Get tag name (e.g. "div", "agent", "br")
    pub fn tag_name(&self) -> &'arena str {
        self.tag_name
    }

    /// Get attributes as key-value pairs
    pub fn attributes(&self) -> &HashMap<&'arena str, &'arena str> {
        &self.attributes
    }

    /// Whether this is a self-closing tag (e.g. `<br/>`, `<img ...>`)
    pub fn is_self_closing(&self) -> bool {
        self.is_self_closing
    }

    /// Whether this tag has no matching closing tag
    pub fn is_unclosed(&self) -> bool {
        self.is_unclosed
    }

    /// Text content between opening and closing tags, if applicable
    pub fn content(&self) -> Option<&'arena str> {
        self.content
    }

    /// Get range in source document
    pub fn range(&self) -> Range {
        self.range
    }
}

// ============================================================================
// ARENA ALLOCATION TESTS
// These tests define the expected behavior for arena-allocated types.
// All tests should FAIL until arena allocation is implemented (RED phase).
// ============================================================================

#[cfg(test)]
#[allow(unused_variables)] // RED phase: arena variables are placeholders until types are migrated
#[allow(clippy::items_after_test_module)] // impl blocks exist after test module - will be fixed in refactor
mod arena_allocation_tests {
    use super::*;
    use bumpalo::Bump;

    // ========================================================================
    // PARSER TYPES: Arena Lifetime Tests
    // ========================================================================

    /// Heading should be arena-allocated with 'arena lifetime
    #[test]
    fn heading_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Heading has lifetime parameter
        let _heading: Heading = Heading {
            level: 1,
            text: arena.alloc_str("Test"),
            range: Range::new(Position::new(0, 0), Position::new(0, 4)),
        };

        // Arena-allocated heading borrows from arena
        panic!("RED: Heading needs 'arena lifetime parameter");
    }

    /// Paragraph should be arena-allocated with 'arena lifetime
    #[test]
    fn paragraph_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Paragraph has lifetime parameter
        let _paragraph: Paragraph = Paragraph {
            text: arena.alloc_str("Test paragraph"),
            range: Range::new(Position::new(0, 0), Position::new(0, 14)),
        };

        panic!("RED: Paragraph needs 'arena lifetime parameter");
    }

    /// ListItem should be arena-allocated with 'arena lifetime
    #[test]
    fn list_item_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, ListItem has lifetime parameter
        let _item: ListItem = ListItem {
            text: arena.alloc_str("- test item"),
            properties_map: HashMap::new(),
            children_list: &[],
        };

        panic!("RED: ListItem needs 'arena lifetime parameter");
    }

    /// WikiLink should be arena-allocated with 'arena lifetime
    #[test]
    fn wiki_link_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, WikiLink has lifetime parameter
        let _link: WikiLink = WikiLink::new(
            arena.alloc_str("target"),
            None,
            None,
            None,
            Range::new(Position::new(0, 0), Position::new(0, 6)),
        );

        panic!("RED: WikiLink needs 'arena lifetime parameter");
    }

    /// MarkdownLink should be arena-allocated with 'arena lifetime
    #[test]
    fn markdown_link_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, MarkdownLink has lifetime parameter
        let _link: MarkdownLink = MarkdownLink::new(
            arena.alloc_str("text"),
            arena.alloc_str("url"),
            None,
            None,
            Range::new(Position::new(0, 0), Position::new(0, 4)),
        );

        panic!("RED: MarkdownLink needs 'arena lifetime parameter");
    }

    /// LinkDefinition should be arena-allocated with 'arena lifetime
    #[test]
    fn link_definition_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, LinkDefinition has lifetime parameter
        let _def: LinkDefinition = LinkDefinition::new(
            arena.alloc_str("label"),
            arena.alloc_str("url"),
            None,
        );

        panic!("RED: LinkDefinition needs 'arena lifetime parameter");
    }

    /// BlockId should be arena-allocated with 'arena lifetime
    #[test]
    fn block_id_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, BlockId has lifetime parameter
        let _id: BlockId = BlockId::new(arena.alloc_str("abc123"));

        panic!("RED: BlockId needs 'arena lifetime parameter");
    }

    /// BlockRef should be arena-allocated with 'arena lifetime
    #[test]
    fn block_ref_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, BlockRef has lifetime parameter
        let _ref: BlockRef = BlockRef::new(arena.alloc_str("uuid-1234"));

        panic!("RED: BlockRef needs 'arena lifetime parameter");
    }

    /// Tag should be arena-allocated with 'arena lifetime
    #[test]
    fn tag_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Tag has lifetime parameter
        let _tag: Tag = Tag::new(arena.alloc_str("project/feature"));

        panic!("RED: Tag needs 'arena lifetime parameter");
    }

    /// Embed should be arena-allocated with 'arena lifetime
    #[test]
    fn embed_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Embed has lifetime parameter
        let _embed: Embed = Embed::new(arena.alloc_str("embedded-page"));

        panic!("RED: Embed needs 'arena lifetime parameter");
    }

    /// Task should be arena-allocated with 'arena lifetime
    #[test]
    fn task_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Task has lifetime parameter
        let _task: Task = Task::new(TaskState::new(arena.alloc_str("TODO")));

        panic!("RED: Task needs 'arena lifetime parameter");
    }

    /// Callout should be arena-allocated with 'arena lifetime
    #[test]
    fn callout_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Callout has lifetime parameter
        let _callout: Callout = Callout::new(arena.alloc_str("note"), Some(arena.alloc_str("Tip")));

        panic!("RED: Callout needs 'arena lifetime parameter");
    }

    /// QueryBlock should be arena-allocated with 'arena lifetime
    #[test]
    fn query_block_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, QueryBlock has lifetime parameter
        let _query: QueryBlock = QueryBlock::new(arena.alloc_str("{{query todo}}"));

        panic!("RED: QueryBlock needs 'arena lifetime parameter");
    }

    /// Frontmatter should be arena-allocated with 'arena lifetime
    #[test]
    fn frontmatter_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Frontmatter has lifetime parameter
        let _fm: Frontmatter = Frontmatter::new(HashMap::new());

        panic!("RED: Frontmatter needs 'arena lifetime parameter");
    }

    /// Properties should be arena-allocated with 'arena lifetime
    #[test]
    fn properties_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Properties has lifetime parameter
        let _props: Properties = Properties::new(HashMap::new());

        panic!("RED: Properties needs 'arena lifetime parameter");
    }

    /// XmlTag should be arena-allocated with 'arena lifetime
    #[test]
    fn xml_tag_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, XmlTag has lifetime parameter
        let _tag: XmlTag = XmlTag::new(
            arena.alloc_str("agent"),
            HashMap::new(),
            false,
            Some(arena.alloc_str("content")),
            Range::new(Position::new(0, 0), Position::new(0, 10)),
        );

        panic!("RED: XmlTag needs 'arena lifetime parameter");
    }

    /// Element enum should be arena-allocated with 'arena lifetime
    #[test]
    fn element_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Element has lifetime parameter
        let _element: Element = Element::Other;

        panic!("RED: Element needs 'arena lifetime parameter");
    }

    // ========================================================================
    // ARENA STRING STORAGE TESTS
    // ========================================================================

    /// Heading text should be &str borrowed from arena, not String
    #[test]
    fn heading_text_is_arena_str() {
        let arena = Bump::new();

        // After migration:
        // struct Heading<'arena> {
        //     text: &'arena str,  // NOT String
        //     ...
        // }
        let text: &str = arena.alloc_str("Test Heading");
        let _heading = Heading {
            level: 1,
            text, // Now &'arena str
            range: Range::new(Position::new(0, 0), Position::new(0, 12)),
        };

        panic!("RED: Heading::text should be &'arena str, not String");
    }

    /// ListItem properties should use arena-allocated HashMap
    #[test]
    fn list_item_properties_arena_map() {
        let arena = Bump::new();

        // After migration, HashMap uses arena-allocated keys/values
        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert(arena.alloc_str("key"), arena.alloc_str("value"));

        // Now using arena-allocated HashMap
        panic!("RED: ListItem::properties_map should use HashMap<&'arena str, &'arena str>");
    }

    /// Vec fields should be arena slices
    #[test]
    fn vec_fields_become_arena_slices() {
        let arena = Bump::new();

        // After migration:
        // children_list: &'arena [ListItem<'arena>]
        let _item = ListItem {
            text: arena.alloc_str("test"),
            properties_map: HashMap::new(),
            children_list: &[], // Now &'arena [ListItem<'arena>]
        };

        panic!("RED: Vec fields should be &'arena [T] slices");
    }
}
