//! Abstract Syntax Tree for arena-allocated markdown parsing.

use markymark_core::arena::DocumentArena;
use markymark_core::prelude::*;
use self_cell::self_cell;
use tree_sitter_md::MarkdownTree;

use crate::types::*;
use tree_sitter::Node;

#[derive(Debug)]
struct AstOwner {
    source: String,
    arena: Box<DocumentArena>,
}

#[derive(Debug)]
struct AstDependent<'a> {
    root_elements: Vec<Element<'a>>,
}

self_cell!(
    struct AstCell {
        owner: AstOwner,

        #[covariant]
        dependent: AstDependent,
    }

    impl { Debug }
);

/// Abstract Syntax Tree representing a parsed markdown document.
///
/// The AST owns a [`DocumentArena`] through a `self_cell` owner/dependent pair:
/// - owner: source text + arena
/// - dependent: root elements borrowing from the arena
///
/// Public accessors borrow data through `&self`, preventing arena-backed
/// references from escaping the `Ast` lifetime in safe code.
pub struct Ast {
    /// Self-referential owner/dependent cell:
    /// - owner: source text + arena
    /// - dependent: root elements borrowing from owner arena
    cell: AstCell,
    /// Tree-sitter-md parse tree (block + inline trees).
    /// Wrapped in `Option` so it can be taken out via [`take_md_tree`](Self::take_md_tree)
    /// for incremental parsing reuse while letting the rest of the AST be consumed.
    md_tree: Option<MarkdownTree>,
}

impl Ast {
    /// Get a reference to the inner bump allocator.
    ///
    /// Callers (e.g. `DocumentIndex`) use this to allocate into the parser's
    /// arena and then take ownership via [`into_arena`](Self::into_arena).
    #[inline]
    pub fn arena(&self) -> &bumpalo::Bump {
        self.cell.borrow_owner().arena.bump()
    }

    /// Consume the AST and return its [`DocumentArena`].
    ///
    /// Used by `DocumentIndex` to take ownership of the arena after borrowing
    /// from it during index construction, avoiding string reallocation.
    pub fn into_arena(self) -> DocumentArena {
        *self.cell.into_owner().arena
    }

    /// Raw pointer to the owned [`DocumentArena`] for extraction when the
    /// borrow checker prevents using [`into_arena`](Self::into_arena).
    ///
    /// Used by `DocumentIndex::from_ast` to borrow and then take ownership
    /// of the arena in a single pass via `ptr::read` + `mem::forget`.
    #[inline]
    pub fn doc_arena_ptr(&self) -> *const DocumentArena {
        &*self.cell.borrow_owner().arena as *const DocumentArena
    }

    /// Create AST from a MarkdownTree (block + inline trees)
    pub(crate) fn from_markdown_tree(md_tree: MarkdownTree, source: &str) -> CoreResult<Self> {
        let owner = AstOwner {
            source: source.to_string(),
            arena: Box::new(DocumentArena::new()),
        };
        let root_node = md_tree.block_tree().root_node();
        let cell = AstCell::try_new(owner, |owner| {
            let mut root_elements = Vec::new();
            // tree-sitter-md wraps content in section nodes:
            // document → section → {atx_heading, paragraph, list, section(nested)}
            collect_elements(
                root_node,
                &owner.source,
                owner.arena.bump(),
                &mut root_elements,
            )?;
            Ok(AstDependent { root_elements })
        })?;

        Ok(Self {
            cell,
            md_tree: Some(md_tree),
        })
    }

    /// Get root-level elements
    ///
    /// ```compile_fail
    /// use markymark_parser::Parser;
    ///
    /// fn leak_heading_text() -> &'static str {
    ///     let mut parser = Parser::new().unwrap();
    ///     let ast = parser.parse("# Title").unwrap();
    ///     ast.root_elements()[0].as_heading().unwrap().text()
    /// }
    /// ```
    pub fn root_elements<'a>(&'a self) -> &'a [Element<'a>] {
        self.cell.borrow_dependent().root_elements.as_slice()
    }

    /// Extract all wiki links from the document
    pub fn extract_wiki_links<'a>(&'a self) -> Vec<WikiLink<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_wiki_links(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Extract all markdown links
    pub fn extract_markdown_links<'a>(&'a self) -> Vec<MarkdownLink<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_markdown_links(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Extract all link definitions
    pub fn extract_link_definitions<'a>(&'a self) -> Vec<LinkDefinition<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_link_definitions(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Extract all block IDs (Obsidian)
    pub fn extract_block_ids<'a>(&'a self) -> Vec<BlockId<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_block_ids(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }

    /// Extract all block references (Logseq)
    pub fn extract_block_refs<'a>(&'a self) -> Vec<BlockRef<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_block_refs(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Extract all tags
    pub fn extract_tags<'a>(&'a self) -> Vec<Tag<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_tags(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }

    /// Extract all embeds
    pub fn extract_embeds<'a>(&'a self) -> Vec<Embed<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_embeds(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }

    /// Extract all list items as references into the arena (avoids cloning ArenaHashMap).
    ///
    /// Returns an empty vec if the tree has been taken via [`take_md_tree`](Self::take_md_tree).
    pub fn extract_list_items<'a>(&'a self) -> Vec<&'a ListItem<'a>> {
        let md_tree = match &self.md_tree {
            Some(t) => t,
            None => return Vec::new(),
        };
        let root_node = md_tree.block_tree().root_node();
        let mut items = Vec::new();
        let owner = self.cell.borrow_owner();
        collect_top_level_list_items(root_node, &owner.source, owner.arena.bump(), &mut items);
        items
    }

    /// Take the `MarkdownTree` out of this AST for external storage.
    ///
    /// After calling this, [`extract_list_items`](Self::extract_list_items) returns empty.
    /// All other extraction methods still work (they use `root_elements`, not the tree).
    ///
    /// Used by the LSP server to store the tree per-document for incremental parsing.
    pub fn take_md_tree(&mut self) -> Option<MarkdownTree> {
        self.md_tree.take()
    }

    /// Extract all tasks
    pub fn extract_tasks<'a>(&'a self) -> Vec<Task<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_tasks(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }

    /// Extract all callouts (Obsidian)
    pub fn extract_callouts<'a>(&'a self) -> Vec<Callout<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_callouts(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }

    /// Extract all query blocks (Logseq)
    pub fn extract_query_blocks<'a>(&'a self) -> Vec<QueryBlock<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_query_blocks(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Get frontmatter if present
    pub fn frontmatter<'a>(&'a self) -> Option<Frontmatter<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_frontmatter(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Get page properties (Logseq)
    pub fn page_properties<'a>(&'a self) -> Option<Properties<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_page_properties(
                &dep.root_elements,
                &owner.source,
                owner.arena.bump(),
            )
        })
    }

    /// Extract all XML/HTML tags from the document
    pub fn extract_xml_tags<'a>(&'a self) -> Vec<XmlTag<'a>> {
        self.cell.with_dependent(|owner, dep| {
            crate::extract::extract_xml_tags(&dep.root_elements, &owner.source, owner.arena.bump())
        })
    }
}

impl Default for Ast {
    fn default() -> Self {
        // Create a minimal empty AST for testing purposes
        // This parses an empty string which produces an empty document
        let mut parser = crate::Parser::new().expect("parser should initialize");
        parser.parse("").expect("empty document should parse")
    }
}

/// Recursively collect elements from the block tree, descending into section nodes.
///
/// tree-sitter-md wraps content in `section` nodes that nest by heading level.
/// This function flattens the section hierarchy to extract elements.
fn collect_elements<'a>(
    node: Node,
    source: &str,
    arena: &'a bumpalo::Bump,
    elements: &mut Vec<Element<'a>>,
) -> CoreResult<()> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Recurse into section nodes (tree-sitter-md's structural wrapper)
        if child.kind() == "section" {
            collect_elements(child, source, arena, elements)?;
            continue;
        }

        if let Some(element) = Element::from_node(child, source, arena)? {
            elements.push(element);
            continue;
        }

        // tree-sitter-md uses "list" instead of tight_list/loose_list
        if child.kind() == "list" {
            let mut list_cursor = child.walk();
            for list_child in child.children(&mut list_cursor) {
                // Logseq-style headings: list items starting with `- # Heading`
                if let Some(heading) = try_logseq_heading(list_child, source, arena) {
                    elements.push(Element::Heading(heading));
                    continue;
                }
                if let Some(element) = Element::from_node(list_child, source, arena)? {
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
fn try_logseq_heading<'a>(
    node: Node,
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Option<Heading<'a>> {
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

    let heading_text = rest[1..].trim();
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

    // Allocate heading text in arena
    let heading_text = arena.alloc_str(heading_text);

    Some(Heading::new(level as u8, heading_text, range))
}

fn collect_top_level_list_items<'a>(
    node: Node,
    source: &str,
    arena: &'a bumpalo::Bump,
    items: &mut Vec<&'a ListItem<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // tree-sitter-md uses "list" instead of tight_list/loose_list
        if child.kind() == "list" {
            if let Ok(list_items) = ListItem::list_items_from_list_node(child, source, arena) {
                // Collect references instead of cloning to avoid ArenaHashMap::clone
                for item in list_items {
                    items.push(item);
                }
            }
            continue;
        }

        // Recurse into section nodes (tree-sitter-md's structural wrapper)
        if child.kind() == "section" {
            collect_top_level_list_items(child, source, arena, items);
            continue;
        }

        collect_top_level_list_items(child, source, arena, items);
    }
}
