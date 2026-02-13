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
