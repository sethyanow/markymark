use markymark_core::prelude::*;
use std::collections::HashMap;
use tree_sitter::Node;

/// An element in the markdown AST
#[derive(Debug, Clone)]
pub enum Element {
    /// Heading element
    Heading(Heading),
    /// Paragraph element
    Paragraph(Paragraph),
    /// List item
    ListItem(ListItem),
    /// Other elements (placeholder for now)
    Other,
}

impl Element {
    pub(crate) fn from_node(node: Node, source: &str) -> CoreResult<Option<Self>> {
        match node.kind() {
            "atx_heading" | "setext_heading" => {
                Ok(Some(Element::Heading(Heading::from_node(node, source)?)))
            }
            "paragraph" => Ok(Some(Element::Paragraph(Paragraph::from_node(
                node, source,
            )?))),
            "list_item" => Ok(Some(Element::ListItem(ListItem::from_node(node, source)?))),
            _ => Ok(None), // Skip unknown node types for now
        }
    }

    /// Try to get as heading
    pub fn as_heading(&self) -> Option<&Heading> {
        match self {
            Element::Heading(h) => Some(h),
            _ => None,
        }
    }

    /// Try to get as paragraph
    pub fn as_paragraph(&self) -> Option<&Paragraph> {
        match self {
            Element::Paragraph(p) => Some(p),
            _ => None,
        }
    }
}

/// A heading
#[derive(Debug, Clone)]
pub struct Heading {
    level: u8,
    text: String,
    range: Range,
}

impl Heading {
    fn from_node(node: Node, source: &str) -> CoreResult<Self> {
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

        Ok(Self { level, text, range })
    }

    /// Get heading level (1-6)
    pub fn level(&self) -> u8 {
        self.level
    }

    /// Get heading text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }

    /// Create a heading directly (used for Logseq-style headings in list items).
    pub(crate) fn new(level: u8, text: String, range: Range) -> Self {
        Self { level, text, range }
    }
}

/// A paragraph
#[derive(Debug, Clone)]
pub struct Paragraph {
    text: String,
    #[allow(dead_code)]
    range: Range,
}

impl Paragraph {
    fn from_node(node: Node, source: &str) -> CoreResult<Self> {
        let text = node
            .utf8_text(source.as_bytes())
            .map_err(|e| CoreError::Message(format!("UTF-8 error: {}", e)))?
            .trim()
            .to_string();

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

        Ok(Self { text, range })
    }

    /// Get paragraph text
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A list item
#[derive(Debug, Clone)]
pub struct ListItem {
    #[allow(dead_code)]
    text: String,
    #[allow(dead_code)]
    properties_map: std::collections::HashMap<String, String>,
    #[allow(dead_code)]
    children_list: Vec<ListItem>,
}

impl ListItem {
    pub(crate) fn from_node(node: Node, source: &str) -> CoreResult<Self> {
        use std::collections::HashMap;

        let text = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .trim()
            .to_string();

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

            properties_map.insert(key.to_string(), value.to_string());
        }

        Ok(Self {
            text,
            properties_map,
            children_list: Vec::new(),
        })
    }

    pub(crate) fn list_items_from_list_node(node: Node, source: &str) -> CoreResult<Vec<ListItem>> {
        let mut items = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "list_item" {
                let mut item = Self::from_node(child, source)?;

                if let Some(sublist) = Self::find_first_list_descendant(child) {
                    item.children_list = Self::list_items_from_list_node(sublist, source)?;
                }

                items.push(item);
            }
        }

        Ok(items)
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
    pub fn properties(&self) -> &std::collections::HashMap<String, String> {
        &self.properties_map
    }

    /// Get child list items
    pub fn children(&self) -> Option<&[ListItem]> {
        if self.children_list.is_empty() {
            None
        } else {
            Some(&self.children_list)
        }
    }
}

// Placeholder types for extraction (minimal implementations)

/// A wiki link
#[derive(Debug, Clone)]
pub struct WikiLink {
    target: String,
    alias: Option<String>,
    heading: Option<String>,
    block_id: Option<String>,
    range: Range,
}

impl WikiLink {
    /// Create a new wiki link
    pub(crate) fn new(
        target: String,
        alias: Option<String>,
        heading: Option<String>,
        block_id: Option<String>,
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
    pub fn target_page(&self) -> Option<&str> {
        if self.target.is_empty() {
            None
        } else {
            Some(&self.target)
        }
    }

    /// Get alias
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    /// Get target heading
    pub fn target_heading(&self) -> Option<&str> {
        self.heading.as_deref()
    }

    /// Get target block ID
    pub fn target_block_id(&self) -> Option<&str> {
        self.block_id.as_deref()
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A markdown link
#[derive(Debug, Clone)]
pub struct MarkdownLink {
    text: String,
    url: String,
    anchor: Option<String>,
    reference: Option<String>,
    range: Range,
}

impl MarkdownLink {
    /// Create a new markdown link
    pub(crate) fn new(
        text: String,
        url: String,
        anchor: Option<String>,
        reference: Option<String>,
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
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get anchor
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    /// Get reference
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A link definition
#[derive(Debug, Clone)]
pub struct LinkDefinition {
    label: String,
    url: String,
    title: Option<String>,
}

impl LinkDefinition {
    /// Create a new link definition
    pub(crate) fn new(label: String, url: String, title: Option<String>) -> Self {
        Self { label, url, title }
    }

    /// Get label
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Get URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get title
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// Block ID (Obsidian)
#[derive(Debug, Clone)]
pub struct BlockId {
    id: String,
}

impl BlockId {
    /// Create a new block ID
    pub(crate) fn new(id: String) -> Self {
        Self { id }
    }

    /// Get ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Block reference (Logseq)
#[derive(Debug, Clone)]
pub struct BlockRef {
    uuid: String,
}

impl BlockRef {
    /// Create a new block reference
    pub(crate) fn new(uuid: String) -> Self {
        Self { uuid }
    }

    /// Get UUID
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}

/// A tag
#[derive(Debug, Clone)]
pub struct Tag {
    name: String,
}

impl Tag {
    /// Create a new tag
    pub(crate) fn new(name: String) -> Self {
        Self { name }
    }

    /// Get tag name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get tag segments (for nested tags like #project/feature/bug)
    pub fn segments(&self) -> Vec<&str> {
        self.name.split('/').collect()
    }
}

/// An embed
#[derive(Debug, Clone)]
pub struct Embed {
    target: String,
}

impl Embed {
    /// Create a new embed
    pub(crate) fn new(target: String) -> Self {
        Self { target }
    }

    /// Get target
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Check if this is an embed
    pub fn is_embed(&self) -> bool {
        true
    }
}

/// A task
#[derive(Debug, Clone)]
pub struct Task {
    state: TaskState,
}

impl Task {
    /// Create a new task
    pub(crate) fn new(state: TaskState) -> Self {
        Self { state }
    }

    /// Get task state
    pub fn state(&self) -> &TaskState {
        &self.state
    }
}

/// Task state
#[derive(Debug, Clone)]
pub struct TaskState {
    name: String,
}

impl TaskState {
    /// Create a new task state
    pub(crate) fn new(name: String) -> Self {
        Self { name }
    }

    /// Get state as string
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

/// Callout (Obsidian)
#[derive(Debug, Clone)]
pub struct Callout {
    callout_type: String,
    title: Option<String>,
}

impl Callout {
    /// Create a new callout
    pub(crate) fn new(callout_type: String, title: Option<String>) -> Self {
        Self {
            callout_type,
            title,
        }
    }

    /// Get callout type
    pub fn callout_type(&self) -> &str {
        &self.callout_type
    }

    /// Get title
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// Query block (Logseq)
#[derive(Debug, Clone)]
pub struct QueryBlock {
    query: String,
}

impl QueryBlock {
    /// Create a new query block
    pub(crate) fn new(query: String) -> Self {
        Self { query }
    }

    /// Get query text
    pub fn query_text(&self) -> &str {
        &self.query
    }
}

/// Frontmatter
#[derive(Debug, Clone)]
pub struct Frontmatter {
    data: std::collections::HashMap<String, FrontmatterValue>,
}

impl Frontmatter {
    /// Create new frontmatter
    pub(crate) fn new(data: std::collections::HashMap<String, FrontmatterValue>) -> Self {
        Self { data }
    }

    /// Get string value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    /// Get list value
    pub fn get_list(&self, key: &str) -> Option<Vec<&str>> {
        self.data.get(key).and_then(|v| v.as_list())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FrontmatterValue {
    String(String),
    List(Vec<String>),
}

impl FrontmatterValue {
    fn as_string(&self) -> Option<&str> {
        match self {
            FrontmatterValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<Vec<&str>> {
        match self {
            FrontmatterValue::List(list) => Some(list.iter().map(|s| s.as_str()).collect()),
            _ => None,
        }
    }
}

/// Properties (Logseq)
#[derive(Debug, Clone)]
pub struct Properties {
    data: std::collections::HashMap<String, PropertyValue>,
}

impl Properties {
    /// Create new properties
    pub(crate) fn new(data: std::collections::HashMap<String, PropertyValue>) -> Self {
        Self { data }
    }

    /// Get property
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.data.get(key)
    }
}

/// A property value (Logseq)
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// String value
    String(String),
    /// List of values
    List(Vec<String>),
    /// Page reference
    PageRef(String),
}

impl PropertyValue {
    /// Get as string
    pub fn as_str(&self) -> &str {
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
pub struct XmlTag {
    tag_name: String,
    attributes: HashMap<String, String>,
    is_self_closing: bool,
    is_unclosed: bool,
    content: Option<String>,
    range: Range,
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

        // After migration, Heading should have lifetime parameter
        // This test FAILS because Heading<'arena> doesn't exist yet
        let _heading: Heading = Heading {
            level: 1,
            text: String::from("Test"),
            range: Range::new(Position::new(0, 0), Position::new(0, 4)),
        };

        // EXPECTED: let heading: &Heading<'arena> = arena.alloc(Heading { ... });
        // Arena-allocated heading should borrow from arena
        panic!("RED: Heading needs 'arena lifetime parameter");
    }

    /// Paragraph should be arena-allocated with 'arena lifetime
    #[test]
    fn paragraph_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Paragraph should have lifetime parameter
        let _paragraph: Paragraph = Paragraph {
            text: String::from("Test paragraph"),
            range: Range::new(Position::new(0, 0), Position::new(0, 14)),
        };

        panic!("RED: Paragraph needs 'arena lifetime parameter");
    }

    /// ListItem should be arena-allocated with 'arena lifetime
    #[test]
    fn list_item_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, ListItem should have lifetime parameter
        let _item: ListItem = ListItem {
            text: String::from("- test item"),
            properties_map: HashMap::new(),
            children_list: Vec::new(),
        };

        panic!("RED: ListItem needs 'arena lifetime parameter");
    }

    /// WikiLink should be arena-allocated with 'arena lifetime
    #[test]
    fn wiki_link_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, WikiLink should have lifetime parameter
        let _link: WikiLink = WikiLink::new(
            String::from("target"),
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

        // After migration, MarkdownLink should have lifetime parameter
        let _link: MarkdownLink = MarkdownLink::new(
            String::from("text"),
            String::from("url"),
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

        // After migration, LinkDefinition should have lifetime parameter
        let _def: LinkDefinition = LinkDefinition::new(
            String::from("label"),
            String::from("url"),
            None,
        );

        panic!("RED: LinkDefinition needs 'arena lifetime parameter");
    }

    /// BlockId should be arena-allocated with 'arena lifetime
    #[test]
    fn block_id_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, BlockId should have lifetime parameter
        let _id: BlockId = BlockId::new(String::from("abc123"));

        panic!("RED: BlockId needs 'arena lifetime parameter");
    }

    /// BlockRef should be arena-allocated with 'arena lifetime
    #[test]
    fn block_ref_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, BlockRef should have lifetime parameter
        let _ref: BlockRef = BlockRef::new(String::from("uuid-1234"));

        panic!("RED: BlockRef needs 'arena lifetime parameter");
    }

    /// Tag should be arena-allocated with 'arena lifetime
    #[test]
    fn tag_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Tag should have lifetime parameter
        let _tag: Tag = Tag::new(String::from("project/feature"));

        panic!("RED: Tag needs 'arena lifetime parameter");
    }

    /// Embed should be arena-allocated with 'arena lifetime
    #[test]
    fn embed_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Embed should have lifetime parameter
        let _embed: Embed = Embed::new(String::from("embedded-page"));

        panic!("RED: Embed needs 'arena lifetime parameter");
    }

    /// Task should be arena-allocated with 'arena lifetime
    #[test]
    fn task_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Task should have lifetime parameter
        let _task: Task = Task::new(TaskState::new(String::from("TODO")));

        panic!("RED: Task needs 'arena lifetime parameter");
    }

    /// Callout should be arena-allocated with 'arena lifetime
    #[test]
    fn callout_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Callout should have lifetime parameter
        let _callout: Callout = Callout::new(String::from("note"), Some(String::from("Tip")));

        panic!("RED: Callout needs 'arena lifetime parameter");
    }

    /// QueryBlock should be arena-allocated with 'arena lifetime
    #[test]
    fn query_block_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, QueryBlock should have lifetime parameter
        let _query: QueryBlock = QueryBlock::new(String::from("{{query todo}}"));

        panic!("RED: QueryBlock needs 'arena lifetime parameter");
    }

    /// Frontmatter should be arena-allocated with 'arena lifetime
    #[test]
    fn frontmatter_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Frontmatter should have lifetime parameter
        let _fm: Frontmatter = Frontmatter::new(HashMap::new());

        panic!("RED: Frontmatter needs 'arena lifetime parameter");
    }

    /// Properties should be arena-allocated with 'arena lifetime
    #[test]
    fn properties_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Properties should have lifetime parameter
        let _props: Properties = Properties::new(HashMap::new());

        panic!("RED: Properties needs 'arena lifetime parameter");
    }

    /// XmlTag should be arena-allocated with 'arena lifetime
    #[test]
    fn xml_tag_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, XmlTag should have lifetime parameter
        let _tag: XmlTag = XmlTag::new(
            String::from("agent"),
            HashMap::new(),
            false,
            Some(String::from("content")),
            Range::new(Position::new(0, 0), Position::new(0, 10)),
        );

        panic!("RED: XmlTag needs 'arena lifetime parameter");
    }

    /// Element enum should be arena-allocated with 'arena lifetime
    #[test]
    fn element_uses_arena_lifetime() {
        let arena = Bump::new();

        // After migration, Element should have lifetime parameter
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
        let text: String = String::from("Test Heading");
        let _heading = Heading {
            level: 1,
            text,  // Should be &'arena str
            range: Range::new(Position::new(0, 0), Position::new(0, 12)),
        };

        panic!("RED: Heading::text should be &'arena str, not String");
    }

    /// ListItem properties should use arena-allocated HashMap
    #[test]
    fn list_item_properties_arena_map() {
        // After migration, HashMap should use bumpalo allocator
        // use hashbrown::HashMap;
        // type ArenaMap<'a, K, V> = HashMap<K, V, bumpalo::collections::allocator::Allocator>;

        let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        map.insert("key".to_string(), "value".to_string());

        // Should use hashbrown with arena allocator instead
        panic!("RED: ListItem::properties_map should use hashbrown::HashMap with bumpalo allocator");
    }

    /// Vec fields should be arena slices
    #[test]
    fn vec_fields_become_arena_slices() {
        let arena = Bump::new();

        // After migration:
        // children_list: &'arena [ListItem<'arena>]
        let _item = ListItem {
            text: String::from("test"),
            properties_map: HashMap::new(),
            children_list: Vec::new(),  // Should be &'arena [ListItem<'arena>]
        };

        panic!("RED: Vec fields should be &'arena [T] slices");
    }
}

impl XmlTag {
    /// Create a new XML tag
    pub(crate) fn new(
        tag_name: String,
        attributes: HashMap<String, String>,
        is_self_closing: bool,
        content: Option<String>,
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
        tag_name: String,
        attributes: HashMap<String, String>,
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
    pub fn tag_name(&self) -> &str {
        &self.tag_name
    }

    /// Get attributes as key-value pairs
    pub fn attributes(&self) -> &HashMap<String, String> {
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
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Get range in source document
    pub fn range(&self) -> Range {
        self.range
    }
}
