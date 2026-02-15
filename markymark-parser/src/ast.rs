use markymark_core::prelude::*;
use tree_sitter_md::MarkdownTree;

use crate::types::*;
use tree_sitter::Node;

/// Abstract Syntax Tree representing a parsed markdown document
pub struct Ast {
    source: String,
    #[allow(dead_code)]
    md_tree: MarkdownTree,
    root_elements: Vec<Element>,
}

impl Ast {
    /// Create AST from a MarkdownTree (block + inline trees)
    pub(crate) fn from_markdown_tree(md_tree: MarkdownTree, source: &str) -> CoreResult<Self> {
        let root_node = md_tree.block_tree().root_node();
        let mut root_elements = Vec::new();

        // tree-sitter-md wraps content in section nodes:
        // document → section → {atx_heading, paragraph, list, section(nested)}
        collect_elements(root_node, source, &mut root_elements)?;

        Ok(Self {
            source: source.to_string(),
            md_tree,
            root_elements,
        })
    }

    /// Get root-level elements
    pub fn root_elements(&self) -> &[Element] {
        &self.root_elements
    }

    /// Extract all wiki links from the document
    pub fn extract_wiki_links(&self) -> Vec<WikiLink> {
        crate::extract::extract_wiki_links(&self.root_elements, &self.source)
    }

    /// Extract all markdown links
    pub fn extract_markdown_links(&self) -> Vec<MarkdownLink> {
        crate::extract::extract_markdown_links(&self.root_elements, &self.source)
    }

    /// Extract all link definitions
    pub fn extract_link_definitions(&self) -> Vec<LinkDefinition> {
        crate::extract::extract_link_definitions(&self.root_elements, &self.source)
    }

    /// Extract all block IDs (Obsidian)
    pub fn extract_block_ids(&self) -> Vec<BlockId> {
        crate::extract::extract_block_ids(&self.root_elements, &self.source)
    }

    /// Extract all block references (Logseq)
    pub fn extract_block_refs(&self) -> Vec<BlockRef> {
        crate::extract::extract_block_refs(&self.root_elements, &self.source)
    }

    /// Extract all tags
    pub fn extract_tags(&self) -> Vec<Tag> {
        crate::extract::extract_tags(&self.root_elements, &self.source)
    }

    /// Extract all embeds
    pub fn extract_embeds(&self) -> Vec<Embed> {
        crate::extract::extract_embeds(&self.root_elements, &self.source)
    }

    /// Extract all list items
    pub fn extract_list_items(&self) -> Vec<ListItem> {
        let root_node = self.md_tree.block_tree().root_node();
        let mut items = Vec::new();
        collect_top_level_list_items(root_node, &self.source, &mut items);
        items
    }

    /// Extract all tasks
    pub fn extract_tasks(&self) -> Vec<Task> {
        crate::extract::extract_tasks(&self.root_elements, &self.source)
    }

    /// Extract all callouts (Obsidian)
    pub fn extract_callouts(&self) -> Vec<Callout> {
        crate::extract::extract_callouts(&self.root_elements, &self.source)
    }

    /// Extract all query blocks (Logseq)
    pub fn extract_query_blocks(&self) -> Vec<QueryBlock> {
        crate::extract::extract_query_blocks(&self.root_elements, &self.source)
    }

    /// Get frontmatter if present
    pub fn frontmatter(&self) -> Option<Frontmatter> {
        crate::extract::extract_frontmatter(&self.root_elements, &self.source)
    }

    /// Get page properties (Logseq)
    pub fn page_properties(&self) -> Option<Properties> {
        crate::extract::extract_page_properties(&self.root_elements, &self.source)
    }

    /// Extract all XML/HTML tags from the document
    pub fn extract_xml_tags(&self) -> Vec<XmlTag> {
        crate::extract::extract_xml_tags(&self.root_elements, &self.source)
    }
}

/// Recursively collect elements from the block tree, descending into section nodes.
///
/// tree-sitter-md wraps content in `section` nodes that nest by heading level.
/// This function flattens the section hierarchy to extract elements.
fn collect_elements(node: Node, source: &str, elements: &mut Vec<Element>) -> CoreResult<()> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Recurse into section nodes (tree-sitter-md's structural wrapper)
        if child.kind() == "section" {
            collect_elements(child, source, elements)?;
            continue;
        }

        if let Some(element) = Element::from_node(child, source)? {
            elements.push(element);
            continue;
        }

        // tree-sitter-md uses "list" instead of tight_list/loose_list
        if child.kind() == "list" {
            let mut list_cursor = child.walk();
            for list_child in child.children(&mut list_cursor) {
                // Logseq-style headings: list items starting with `- # Heading`
                if let Some(heading) = try_logseq_heading(list_child, source) {
                    elements.push(Element::Heading(heading));
                    continue;
                }
                if let Some(element) = Element::from_node(list_child, source)? {
                    elements.push(element);
                }
            }
        }
    }
    Ok(())
}

/// Detect Logseq-style headings inside list items.
///
/// Logseq markdown prefixes headings with list markers: `- # Heading`, `- ## Sub`.
/// Tree-sitter parses these as list items, not ATX headings. This function checks
/// whether a list_item node contains a heading pattern and extracts it.
fn try_logseq_heading(node: Node, source: &str) -> Option<Heading> {
    if node.kind() != "list_item" {
        return None;
    }

    let node_text = node.utf8_text(source.as_bytes()).ok()?;
    let first_line = node_text.lines().next()?;

    // Strip leading whitespace and list marker (`- `, `* `, `+ `)
    let trimmed = first_line.trim_start();
    let after_marker =
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            &trimmed[2..]
        } else {
            return None;
        };

    // Must start with 1-6 `#` characters followed by a space
    if !after_marker.starts_with('#') {
        return None;
    }
    let level = after_marker.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &after_marker[level..];
    if !rest.starts_with(' ') {
        return None;
    }

    let heading_text = rest[1..].trim().to_string();
    if heading_text.is_empty() {
        return None;
    }

    // Range covers from the `#` markers to end of heading text.
    // Calculate column offset: node start + whitespace + marker length.
    let leading_ws = first_line.len() - trimmed.len();
    let hash_col = node.start_position().column + leading_ws + 2; // +2 for "- "
    let row = node.start_position().row as u32;

    let range = Range::new(
        Position::new(row, hash_col as u32),
        Position::new(row, (hash_col + level + 1 + heading_text.len()) as u32),
    );

    Some(Heading::new(level as u8, heading_text, range))
}

fn collect_top_level_list_items<'a>(node: Node<'a>, source: &str, items: &mut Vec<ListItem>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // tree-sitter-md uses "list" instead of tight_list/loose_list
        if child.kind() == "list" {
            if let Ok(list_items) = ListItem::list_items_from_list_node(child, source) {
                items.extend(list_items);
            }
            continue;
        }

        // Recurse into section nodes (tree-sitter-md's structural wrapper)
        if child.kind() == "section" {
            collect_top_level_list_items(child, source, items);
            continue;
        }

        collect_top_level_list_items(child, source, items);
    }
}
