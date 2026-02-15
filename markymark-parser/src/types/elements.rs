//! Element primitives: Element, Heading, Paragraph, ListItem.

use markymark_core::arena::{new_arena_hashmap, ArenaHashMap};
use markymark_core::prelude::*;
use tree_sitter::Node;

use super::arena_alloc_str;

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
            "atx_heading" | "setext_heading" => Ok(Some(Element::Heading(Heading::from_node(
                node, source, arena,
            )?))),
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
                // Setext: === for h1, --- for h2
                let mut level = 1u8;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "setext_h1_underline" {
                        level = 1;
                        break;
                    } else if child.kind() == "setext_h2_underline" {
                        level = 2;
                        break;
                    }
                }
                level
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

    /// Create a heading directly (used for Logseq-style headings in list items and tests).
    pub(crate) fn new(level: u8, text: &'arena str, range: Range) -> Self {
        Self { level, text, range }
    }
}

/// A paragraph
#[derive(Debug, Clone)]
pub struct Paragraph<'arena> {
    text: &'arena str,
    #[expect(dead_code, reason = "stored for future range-based queries")]
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

    /// Create a paragraph directly (used by tests).
    #[cfg(test)]
    pub(crate) fn new(text: &'arena str, range: Range) -> Self {
        Self { text, range }
    }
}

/// A list item
#[derive(Debug, Clone)]
pub struct ListItem<'arena> {
    text: &'arena str,
    properties_map: ArenaHashMap<'arena, &'arena str, &'arena str>,
    children_list: &'arena [ListItem<'arena>],
}

impl<'arena> ListItem<'arena> {
    pub(crate) fn from_node(
        node: Node,
        source: &str,
        arena: &'arena bumpalo::Bump,
    ) -> CoreResult<Self> {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").trim();

        let mut properties_map = new_arena_hashmap(arena);
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

        let children_list: &'arena [ListItem<'arena>] =
            if let Some(child_list) = Self::find_first_list_descendant(node) {
                Self::list_items_from_list_node(child_list, source, arena)?
            } else {
                bumpalo::collections::Vec::new_in(arena).into_bump_slice()
            };

        Ok(Self {
            text: arena_alloc_str(arena, text),
            properties_map,
            children_list,
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

    /// Get list item text
    pub fn text(&self) -> &'arena str {
        self.text
    }

    /// Get list item properties
    pub fn properties(&self) -> &ArenaHashMap<'arena, &'arena str, &'arena str> {
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

    /// Create a list item directly (used by tests and construction).
    #[cfg(test)]
    pub(crate) fn new(
        text: &'arena str,
        properties_map: ArenaHashMap<'arena, &'arena str, &'arena str>,
        children_list: &'arena [ListItem<'arena>],
    ) -> Self {
        Self {
            text,
            properties_map,
            children_list,
        }
    }
}
