use markymark_core::prelude::*;
use tree_sitter::Tree;

use crate::types::*;
use tree_sitter::Node;

/// Abstract Syntax Tree representing a parsed markdown document
pub struct Ast {
    source: String,
    #[allow(dead_code)]
    tree: Tree,
    root_elements: Vec<Element>,
}

impl Ast {
    /// Create AST from tree-sitter tree
    pub(crate) fn from_tree(tree: Tree, source: &str) -> CoreResult<Self> {
        let root_node = tree.root_node();
        let mut root_elements = Vec::new();

        // Walk the tree and extract elements
        {
            let mut cursor = root_node.walk();
            for child in root_node.children(&mut cursor) {
                if let Some(element) = Element::from_node(child, source)? {
                    root_elements.push(element);
                    continue;
                }

                if child.kind() == "tight_list" || child.kind() == "loose_list" {
                    let mut list_cursor = child.walk();
                    for list_child in child.children(&mut list_cursor) {
                        if let Some(element) = Element::from_node(list_child, source)? {
                            root_elements.push(element);
                        }
                    }
                }
            }
        }

        Ok(Self {
            source: source.to_string(),
            tree,
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
        let root_node = self.tree.root_node();
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

fn collect_top_level_list_items<'a>(node: Node<'a>, source: &str, items: &mut Vec<ListItem>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "tight_list" || child.kind() == "loose_list" {
            if let Ok(list_items) = ListItem::list_items_from_list_node(child, source) {
                items.extend(list_items);
            }

            continue;
        }

        collect_top_level_list_items(child, source, items);
    }
}
