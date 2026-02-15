//! Abstract Syntax Tree for arena-allocated markdown parsing.

use markymark_core::arena::DocumentArena;
use markymark_core::prelude::*;
use tree_sitter_md::MarkdownTree;

use crate::types::*;
use tree_sitter::Node;

/// Abstract Syntax Tree representing a parsed markdown document.
///
/// The AST owns a [`DocumentArena`] that all parsed data is allocated into,
/// enabling efficient bulk deallocation when the AST is dropped.
///
/// # Safety (self-referential arena pattern)
///
/// `root_elements` stores `Element<'static>` but the actual lifetime is the
/// arena's lifetime. This is sound because `Self` owns the arena — the
/// references cannot outlive the struct. The `'static` marker is a workaround
/// for Rust's inability to express self-referential borrows.
///
/// All public accessors return references tied to `&self`, so data cannot
/// outlive the struct in safe code. However, inner types contain `&'static str`
/// fields which technically allow extracting arena references beyond `&self`.
/// Callers **must not** store inner `&'static str` values (e.g.
/// `heading.text()`) past the `Ast` lifetime. A future version will use
/// `self_cell` or `ouroboros` to enforce this statically.
pub struct Ast {
    /// Source text (owned, kept for extract functions)
    source: String,
    /// Root-level elements, allocated in arena (see struct-level safety docs).
    ///
    /// **Drop order**: Must be declared before `arena` so Elements (which contain
    /// `ArenaHashMap`s referencing the arena) are dropped while the arena is alive.
    root_elements: Vec<Element<'static>>,
    /// Per-document arena for all allocated data (boxed for stable address)
    arena: Box<DocumentArena>,
    /// Tree-sitter-md parse tree (block + inline trees) — dropped last
    #[allow(dead_code)]
    md_tree: MarkdownTree,
}

impl Ast {
    /// Get a reference to the inner bump allocator.
    ///
    /// Callers (e.g. `DocumentIndex`) use this to allocate into the parser's
    /// arena and then take ownership via [`into_arena`](Self::into_arena).
    #[inline]
    pub fn arena(&self) -> &bumpalo::Bump {
        self.arena.bump()
    }

    /// Get a `'static` reference to the inner bump allocator.
    ///
    /// # Safety
    ///
    /// The returned reference has `'static` lifetime but is only valid for
    /// the lifetime of `self`. Sound because `self` owns the `DocumentArena`.
    /// Callers must not let the reference escape beyond `&self` methods.
    #[inline]
    fn arena_ref(&self) -> &'static bumpalo::Bump {
        // SAFETY: DocumentArena is owned by Self; reference valid for Self's lifetime.
        unsafe { &*(self.arena.bump() as *const bumpalo::Bump) }
    }

    /// Consume the AST and return its [`DocumentArena`].
    ///
    /// Used by `DocumentIndex` to take ownership of the arena after borrowing
    /// from it during index construction, avoiding string reallocation.
    pub fn into_arena(self) -> DocumentArena {
        *self.arena
    }

    /// Raw pointer to the owned [`DocumentArena`] for extraction when the
    /// borrow checker prevents using [`into_arena`](Self::into_arena).
    ///
    /// Used by `DocumentIndex::from_ast` to borrow and then take ownership
    /// of the arena in a single pass via `ptr::read` + `mem::forget`.
    #[inline]
    pub fn doc_arena_ptr(&self) -> *const DocumentArena {
        &*self.arena as *const DocumentArena
    }

    /// Create AST from a MarkdownTree (block + inline trees)
    pub(crate) fn from_markdown_tree(md_tree: MarkdownTree, source: &str) -> CoreResult<Self> {
        let arena = Box::new(DocumentArena::new());
        let root_node = md_tree.block_tree().root_node();

        // SAFETY: The arena is owned by Self, so the reference is valid for Self's lifetime.
        // See struct-level safety docs for the self-referential pattern rationale.
        let arena_ref: &'static bumpalo::Bump = unsafe { &*(arena.bump() as *const bumpalo::Bump) };

        let mut root_elements = Vec::new();

        // tree-sitter-md wraps content in section nodes:
        // document → section → {atx_heading, paragraph, list, section(nested)}
        collect_elements(root_node, source, arena_ref, &mut root_elements)?;

        Ok(Self {
            source: source.to_string(),
            arena,
            md_tree,
            root_elements,
        })
    }

    /// Get root-level elements
    pub fn root_elements(&self) -> &[Element<'static>] {
        &self.root_elements
    }

    /// Extract all wiki links from the document
    pub fn extract_wiki_links(&self) -> Vec<WikiLink<'static>> {
        crate::extract::extract_wiki_links(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all markdown links
    pub fn extract_markdown_links(&self) -> Vec<MarkdownLink<'static>> {
        crate::extract::extract_markdown_links(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all link definitions
    pub fn extract_link_definitions(&self) -> Vec<LinkDefinition<'static>> {
        crate::extract::extract_link_definitions(
            &self.root_elements,
            &self.source,
            self.arena_ref(),
        )
    }

    /// Extract all block IDs (Obsidian)
    pub fn extract_block_ids(&self) -> Vec<BlockId<'static>> {
        crate::extract::extract_block_ids(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all block references (Logseq)
    pub fn extract_block_refs(&self) -> Vec<BlockRef<'static>> {
        crate::extract::extract_block_refs(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all tags
    pub fn extract_tags(&self) -> Vec<Tag<'static>> {
        crate::extract::extract_tags(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all embeds
    pub fn extract_embeds(&self) -> Vec<Embed<'static>> {
        crate::extract::extract_embeds(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all list items as references into the arena (avoids cloning ArenaHashMap).
    pub fn extract_list_items(&self) -> Vec<&ListItem<'static>> {
        let root_node = self.md_tree.block_tree().root_node();
        let mut items = Vec::new();
        collect_top_level_list_items(root_node, &self.source, self.arena_ref(), &mut items);
        items
    }

    /// Extract all tasks
    pub fn extract_tasks(&self) -> Vec<Task<'static>> {
        crate::extract::extract_tasks(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all callouts (Obsidian)
    pub fn extract_callouts(&self) -> Vec<Callout<'static>> {
        crate::extract::extract_callouts(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all query blocks (Logseq)
    pub fn extract_query_blocks(&self) -> Vec<QueryBlock<'static>> {
        crate::extract::extract_query_blocks(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Get frontmatter if present
    pub fn frontmatter(&self) -> Option<Frontmatter<'static>> {
        crate::extract::extract_frontmatter(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Get page properties (Logseq)
    pub fn page_properties(&self) -> Option<Properties<'static>> {
        crate::extract::extract_page_properties(&self.root_elements, &self.source, self.arena_ref())
    }

    /// Extract all XML/HTML tags from the document
    pub fn extract_xml_tags(&self) -> Vec<XmlTag<'static>> {
        crate::extract::extract_xml_tags(&self.root_elements, &self.source, self.arena_ref())
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
