//! Document indexing: heading lookup, block lookup, TOC, outline tree.

use markymark_core::prelude::*;
use markymark_parser::Ast;
use std::collections::HashMap;

/// A heading entry in the document index.
#[derive(Debug, Clone)]
pub struct HeadingEntry {
    /// The heading text.
    pub text: String,
    /// URL-safe slug derived from the heading text.
    pub slug: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range of the heading.
    pub range: Range,
}

/// A block entry in the document index (Obsidian `^block-id`).
#[derive(Debug, Clone)]
pub struct BlockEntry {
    /// The block identifier.
    pub id: String,
    /// Source range of the block.
    pub range: Range,
}

/// A table-of-contents entry.
#[derive(Debug, Clone)]
pub struct TocEntry {
    /// Heading text.
    pub text: String,
    /// URL-safe slug.
    pub slug: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Nesting depth relative to the root (0-based).
    pub depth: usize,
}

/// A node in the document outline tree.
#[derive(Debug, Clone)]
pub struct OutlineNode {
    /// The heading at this node, if any (root node has `None`).
    pub heading: Option<HeadingEntry>,
    /// Child outline nodes.
    pub children: Vec<OutlineNode>,
}

/// A wiki link entry stored in the index.
#[derive(Debug, Clone)]
pub struct WikiLinkEntry {
    /// Target page name.
    pub target: String,
    /// Optional alias text.
    pub alias: Option<String>,
    /// Optional heading anchor within the target.
    pub heading: Option<String>,
    /// Source range.
    pub range: Range,
}

/// A tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct TagEntry {
    /// Tag name (without leading `#`).
    pub name: String,
}

/// A markdown link entry stored in the index.
#[derive(Debug, Clone)]
pub struct MarkdownLinkEntry {
    /// Link display text.
    pub text: String,
    /// Link URL.
    pub url: String,
    /// Optional anchor/fragment.
    pub anchor: Option<String>,
    /// Source range.
    pub range: Range,
}

/// An XML tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct XmlTagEntry {
    /// Tag name (e.g. "agent", "goal", "task").
    pub tag_name: String,
    /// Tag attributes as key-value pairs.
    pub attributes: HashMap<String, String>,
    /// Whether this is a self-closing tag (e.g. `<br/>`).
    pub is_self_closing: bool,
    /// Source range of the entire tag.
    pub range: Range,
}

/// Convert heading text to a URL-safe slug.
pub fn slugify(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut slug = String::with_capacity(lower.len());

    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '-' {
            slug.push(ch);
        } else if ch == ' ' {
            slug.push('-');
        }
        // Other non-alphanumeric chars are stripped entirely
    }

    // Collapse consecutive dashes
    let mut result = String::with_capacity(slug.len());
    let mut prev_dash = false;
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(ch);
            prev_dash = false;
        }
    }

    // Trim dashes from start/end
    result.trim_matches('-').to_string()
}

/// Deduplicate a slug given a set of already-used slugs.
fn dedup_slug(base: &str, used: &mut HashMap<String, usize>) -> String {
    let count = used.entry(base.to_string()).or_insert(0);
    let slug = if *count == 0 {
        base.to_string()
    } else {
        format!("{}-{}", base, count)
    };
    *count += 1;
    slug
}

/// Index of a single parsed markdown document.
///
/// Built from a [`markymark_parser::Ast`], provides fast lookups for
/// headings (by slug), block IDs, table of contents, and outline tree.
pub struct DocumentIndex {
    headings: Vec<HeadingEntry>,
    slug_to_heading: HashMap<String, usize>,
    blocks: HashMap<String, BlockEntry>,
    toc: Vec<TocEntry>,
    outline: OutlineNode,
    wiki_links: Vec<WikiLinkEntry>,
    tags: Vec<TagEntry>,
    markdown_links: Vec<MarkdownLinkEntry>,
    xml_tags: Vec<XmlTagEntry>,
}

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    pub fn from_ast(ast: &Ast) -> Self {
        let mut headings = Vec::new();
        let mut slug_to_heading = HashMap::new();
        let mut slug_counts: HashMap<String, usize> = HashMap::new();

        // Extract headings
        for element in ast.root_elements() {
            if let Some(h) = element.as_heading() {
                let base_slug = slugify(h.text());
                let slug = dedup_slug(&base_slug, &mut slug_counts);
                let idx = headings.len();
                slug_to_heading.insert(slug.clone(), idx);
                headings.push(HeadingEntry {
                    text: h.text().to_string(),
                    slug,
                    level: h.level(),
                    range: h.range(),
                });
            }
        }

        // Extract block IDs
        let mut blocks = HashMap::new();
        for block_id in ast.extract_block_ids() {
            let id = block_id.id().to_string();
            blocks.insert(
                id.clone(),
                BlockEntry {
                    id,
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                },
            );
        }

        // Build TOC
        let toc = build_toc(&headings);

        // Build outline tree
        let outline = build_outline(&headings);

        // Extract wiki links
        let wiki_links = ast
            .extract_wiki_links()
            .into_iter()
            .map(|wl| WikiLinkEntry {
                target: wl.target_page().unwrap_or("").to_string(),
                alias: wl.alias().map(|s| s.to_string()),
                heading: wl.target_heading().map(|s| s.to_string()),
                range: wl.range(),
            })
            .collect();

        // Extract tags
        let tags = ast
            .extract_tags()
            .into_iter()
            .map(|t| TagEntry {
                name: t.name().to_string(),
            })
            .collect();

        // Extract markdown links
        let markdown_links = ast
            .extract_markdown_links()
            .into_iter()
            .map(|ml| {
                // Reconstruct the full URL including anchor if present
                let url = match ml.anchor() {
                    Some(anchor) => format!("{}#{}", ml.url(), anchor),
                    None => ml.url().to_string(),
                };
                MarkdownLinkEntry {
                    text: ml.text().to_string(),
                    url,
                    anchor: ml.anchor().map(|s| s.to_string()),
                    range: ml.range(),
                }
            })
            .collect();

        // Extract XML tags
        let xml_tags = ast
            .extract_xml_tags()
            .into_iter()
            .map(|xt| XmlTagEntry {
                tag_name: xt.tag_name().to_string(),
                attributes: xt.attributes().clone(),
                is_self_closing: xt.is_self_closing(),
                range: xt.range(),
            })
            .collect();

        Self {
            headings,
            slug_to_heading,
            blocks,
            toc,
            outline,
            wiki_links,
            tags,
            markdown_links,
            xml_tags,
        }
    }

    /// Look up a heading by its slug.
    pub fn heading_by_slug(&self, slug: &str) -> Option<&HeadingEntry> {
        self.slug_to_heading
            .get(slug)
            .map(|&idx| &self.headings[idx])
    }

    /// Look up a block by its ID.
    pub fn block_by_id(&self, id: &str) -> Option<&BlockEntry> {
        self.blocks.get(id)
    }

    /// Get the flat table of contents.
    pub fn toc(&self) -> &[TocEntry] {
        &self.toc
    }

    /// Get the outline tree.
    pub fn outline(&self) -> &OutlineNode {
        &self.outline
    }

    /// Get all indexed headings.
    pub fn headings(&self) -> &[HeadingEntry] {
        &self.headings
    }

    /// Get all indexed wiki links.
    pub fn wiki_links(&self) -> &[WikiLinkEntry] {
        &self.wiki_links
    }

    /// Get all indexed tags.
    pub fn tags(&self) -> &[TagEntry] {
        &self.tags
    }

    /// Get all indexed markdown links.
    pub fn markdown_links(&self) -> &[MarkdownLinkEntry] {
        &self.markdown_links
    }

    /// Get all indexed XML tags.
    pub fn xml_tags(&self) -> &[XmlTagEntry] {
        &self.xml_tags
    }

    /// Get all block IDs in this document.
    pub fn block_ids(&self) -> impl Iterator<Item = &str> {
        self.blocks.keys().map(|s| s.as_str())
    }
}

/// Build flat TOC entries with depth calculation.
fn build_toc(headings: &[HeadingEntry]) -> Vec<TocEntry> {
    let mut toc = Vec::new();
    // Stack tracks heading levels for depth calculation
    let mut level_stack: Vec<u8> = Vec::new();

    for h in headings {
        // Pop levels from stack that are >= current level
        while let Some(&top) = level_stack.last() {
            if top >= h.level {
                level_stack.pop();
            } else {
                break;
            }
        }

        let depth = level_stack.len();
        level_stack.push(h.level);

        toc.push(TocEntry {
            text: h.text.clone(),
            slug: h.slug.clone(),
            level: h.level,
            depth,
        });
    }

    toc
}

/// Build outline tree from heading entries.
fn build_outline(headings: &[HeadingEntry]) -> OutlineNode {
    let mut root = OutlineNode {
        heading: None,
        children: Vec::new(),
    };

    // Stack of (level, node pointer path)
    // We build using a stack approach: track where to insert children
    let mut stack: Vec<(u8, usize)> = Vec::new(); // (level, index_in_parent_children)

    for h in headings {
        let node = OutlineNode {
            heading: Some(h.clone()),
            children: Vec::new(),
        };

        // Pop stack entries where the level is >= current heading level
        while let Some(&(lvl, _)) = stack.last() {
            if lvl >= h.level {
                stack.pop();
            } else {
                break;
            }
        }

        if stack.is_empty() {
            // Insert as child of root
            root.children.push(node);
            let idx = root.children.len() - 1;
            stack.push((h.level, idx));
        } else {
            // Navigate to the parent node and insert as child
            let child_idx = insert_into_outline(&mut root, &stack, node);
            stack.push((h.level, child_idx));
        }
    }

    root
}

/// Insert a node into the outline tree following the stack path.
/// Returns the index of the inserted node in its parent's children.
fn insert_into_outline(root: &mut OutlineNode, stack: &[(u8, usize)], node: OutlineNode) -> usize {
    let mut current = root;
    for &(_, idx) in stack {
        current = &mut current.children[idx];
    }
    current.children.push(node);
    current.children.len() - 1
}
