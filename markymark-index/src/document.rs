//! Document indexing: heading lookup, block lookup, TOC, outline tree.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::prelude::*;
use markymark_parser::Ast;
use std::collections::HashMap as StdHashMap;
use std::fmt;
use std::sync::Mutex;

#[cfg(feature = "zig-kernels")]
use markymark_core::scanner::{ScanBackend, ScanLinkType};

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
}

/// A tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct TagEntry<'arena> {
    /// Tag name (without leading `#`).
    pub name: &'arena str,
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
fn dedup_slug(base: &str, used: &mut StdHashMap<String, usize>) -> String {
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
///
/// # Safety (self-referential arena pattern)
///
/// This struct owns a [`DocumentArena`] and stores arena-allocated data with
/// `'static` lifetime markers. The actual lifetime is the arena's lifetime.
/// All public accessors return references tied to `&self`, so data cannot
/// outlive the struct in safe code. However, inner types contain `&'static str`
/// fields which technically allow extracting arena references beyond `&self`.
/// Callers **must not** store inner `&'static str` values (e.g. `heading.text`)
/// past the `DocumentIndex` lifetime. A future version will use `self_cell` or
/// `ouroboros` to enforce this statically.
///
/// # Why `Mutex<DocumentArena>`
///
/// `Bump: !Sync` makes `DocumentArena: !Sync`, which prevents `DocumentIndex`
/// from implementing `Send + Sync`. tower-lsp requires `Send + 'static` for
/// async handlers that store state in `RwLock<ServerState>`. Wrapping in
/// `Mutex` satisfies `Send + Sync`. The mutex is never locked at runtime —
/// it exists solely for ownership and drop-order correctness.
pub struct DocumentIndex {
    headings: &'static [HeadingEntry<'static>],
    slug_to_heading: HashMap<&'static str, usize>,
    blocks: HashMap<&'static str, BlockEntry<'static>>,
    toc: &'static [TocEntry<'static>],
    outline: OutlineNode<'static>,
    wiki_links: &'static [WikiLinkEntry<'static>],
    tags: &'static [TagEntry<'static>],
    markdown_links: &'static [MarkdownLinkEntry<'static>],
    xml_tags: &'static [XmlTagEntry<'static>],
    /// Arena kept alive so `'static` references in this struct remain valid.
    /// Wrapped in `Mutex` for `Send + Sync`; never locked after construction.
    /// **Drop order**: declared last so all arena-referencing fields are dropped
    /// before the arena memory is freed.
    _arena: Mutex<DocumentArena>,
}

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    ///
    /// Borrows strings from the parser's arena instead of reallocating,
    /// then takes ownership of the arena. Callers pass `ast` by value;
    /// the AST is consumed and its arena is moved into the index.
    pub fn from_ast(ast: Ast) -> Self {
        // Get a 'static reference to the arena's Bump allocator.
        // SAFETY: ast owns the arena; this reference is valid for ast's lifetime.
        // The 'static marker is a workaround for Rust's inability to express
        // self-referential borrows — the actual lifetime is the arena's.
        let arena_ref: &'static Bump = unsafe { &*(ast.arena() as *const Bump) };

        let mut headings_builder: BumpVec<'static, HeadingEntry<'static>> =
            BumpVec::new_in(arena_ref);
        let mut slug_to_heading = HashMap::new();
        let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();

        // Extract headings: borrow text from AST, allocate only slug (computed)
        for element in ast.root_elements() {
            if let Some(h) = element.as_heading() {
                let base_slug = slugify(h.text());
                let slug_owned = dedup_slug(&base_slug, &mut slug_counts);
                let text = h.text(); // borrow from parser arena
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let idx = headings_builder.len();

                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level(),
                    range: h.range(),
                });
            }
        }

        let headings = headings_builder.into_bump_slice();

        // Extract block IDs: borrow from AST, propagate source range for go-to-definition
        let mut blocks = HashMap::new();
        for block_id in ast.extract_block_ids() {
            let id = block_id.id(); // borrow from parser arena
            blocks.insert(
                id,
                BlockEntry {
                    id,
                    range: block_id.range(),
                },
            );
        }

        // Build TOC and outline tree
        let toc = build_toc(arena_ref, headings);
        let outline = build_outline(arena_ref, headings);

        // Extract wiki links: borrow from AST
        let mut wiki_links_builder: BumpVec<'static, WikiLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for wl in ast.extract_wiki_links() {
            // Skip malformed links with no target, heading, or block
            if wl.target_page().is_none()
                && wl.target_heading().is_none()
                && wl.target_block_id().is_none()
            {
                continue;
            }
            wiki_links_builder.push(WikiLinkEntry {
                target: wl.target_page().unwrap_or(""), // "" = current page for heading-only links
                alias: wl.alias(),
                heading: wl.target_heading(),
                range: wl.range(),
            });
        }
        let wiki_links = wiki_links_builder.into_bump_slice();

        // Extract tags: borrow from AST
        let mut tags_builder: BumpVec<'static, TagEntry<'static>> = BumpVec::new_in(arena_ref);
        for t in ast.extract_tags() {
            tags_builder.push(TagEntry { name: t.name() });
        }
        let tags = tags_builder.into_bump_slice();

        // Extract markdown links: borrow from AST (url is base only, anchor separate)
        let mut markdown_links_builder: BumpVec<'static, MarkdownLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for ml in ast.extract_markdown_links() {
            markdown_links_builder.push(MarkdownLinkEntry {
                text: ml.text(),
                url: ml.url(),
                anchor: ml.anchor(),
                range: ml.range(),
            });
        }
        let markdown_links = markdown_links_builder.into_bump_slice();

        // Extract XML tags: borrow from AST.
        // Uses standard HashMap (not ArenaHashMap) because Bump: !Sync
        // makes ArenaHashMap !Send, breaking tower-lsp's Send requirement.
        let mut xml_tags_builder: BumpVec<'static, XmlTagEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for xt in ast.extract_xml_tags() {
            let mut attributes = HashMap::new();
            for (k, v) in xt.attributes() {
                attributes.insert(*k, *v); // borrow from parser arena
            }

            xml_tags_builder.push(XmlTagEntry {
                tag_name: xt.tag_name(),
                attributes,
                is_self_closing: xt.is_self_closing(),
                is_unclosed: xt.is_unclosed(),
                range: xt.range(),
            });
        }
        let xml_tags = xml_tags_builder.into_bump_slice();

        // Transfer arena ownership from the consumed AST.
        // take_arena() destructures the AST and drops non-arena fields in the
        // correct order (root_elements first while Box is alive), then returns
        // the DocumentArena. This replaces the ptr::read + mem::forget pattern
        // which leaked source, root_elements, md_tree, and the Box shell.
        let doc_arena = ast.take_arena();

        Self {
            _arena: Mutex::new(doc_arena),
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

    /// Build a document index from raw text using a scan backend.
    ///
    /// Unlike [`from_ast`](Self::from_ast), this does not parse a full AST.
    /// The scan backend extracts headings, links, tags, and block IDs directly.
    /// XML tags are not extracted (the scan backend does not support them).
    #[cfg(feature = "zig-kernels")]
    pub fn from_scan(text: &str, backend: &dyn ScanBackend) -> Self {
        let doc_arena = DocumentArena::new();
        // SAFETY: We own the arena and move it into the struct at the end.
        // The 'static marker is the same self-referential pattern as from_ast.
        let arena_ref: &'static Bump = unsafe { &*(doc_arena.bump() as *const Bump) };

        // Pre-compute line starts for byte-offset -> Position conversion
        let line_starts = byte_offset_line_starts(text);

        // --- Headings ---
        let mut headings_builder: BumpVec<'static, HeadingEntry<'static>> =
            BumpVec::new_in(arena_ref);
        let mut slug_to_heading = HashMap::new();
        let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();

        if let Ok(scan_headings) = backend.scan_headings(text) {
            for h in scan_headings {
                let base_slug = slugify(&h.text);
                let slug_owned = dedup_slug(&base_slug, &mut slug_counts);
                let heading_text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let pos = byte_offset_to_position(&line_starts, h.offset);
                let end_pos = byte_offset_to_position(
                    &line_starts,
                    h.offset + h.text.len() as u32 + h.level as u32 + 1, // # + space
                );
                let idx = headings_builder.len();

                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text: heading_text,
                    slug,
                    level: h.level,
                    range: Range::new(pos, end_pos),
                });
            }
        }
        let headings = headings_builder.into_bump_slice();

        // --- Links (split into markdown and wiki) ---
        let mut wiki_links_builder: BumpVec<'static, WikiLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);
        let mut markdown_links_builder: BumpVec<'static, MarkdownLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);

        if let Ok(scan_links) = backend.scan_links(text) {
            for l in scan_links {
                let pos = byte_offset_to_position(&line_starts, l.offset);
                let end_offset = l.offset + l.text.len() as u32 + l.target.len() as u32 + 4; // []()
                let end_pos = byte_offset_to_position(&line_starts, end_offset);
                let range = Range::new(pos, end_pos);

                match l.link_type {
                    ScanLinkType::Wiki => {
                        let target = arena_alloc_str(arena_ref, &l.target);
                        // Wiki links: check for alias (target|alias format)
                        let alias = if l.text != l.target {
                            Some(arena_alloc_str(arena_ref, &l.text))
                        } else {
                            None
                        };
                        wiki_links_builder.push(WikiLinkEntry {
                            target,
                            alias,
                            heading: None, // Scan backend doesn't parse heading fragments
                            range,
                        });
                    }
                    ScanLinkType::Markdown => {
                        let link_text = arena_alloc_str(arena_ref, &l.text);
                        // Split URL and anchor
                        let (url_str, anchor) = if let Some(hash_pos) = l.target.find('#') {
                            let url_part = &l.target[..hash_pos];
                            let anchor_part = &l.target[hash_pos + 1..];
                            (url_part, Some(anchor_part))
                        } else {
                            (l.target.as_str(), None)
                        };
                        let url = arena_alloc_str(arena_ref, url_str);
                        let anchor = anchor.map(|a| arena_alloc_str(arena_ref, a));
                        markdown_links_builder.push(MarkdownLinkEntry {
                            text: link_text,
                            url,
                            anchor,
                            range,
                        });
                    }
                }
            }
        }
        let wiki_links = wiki_links_builder.into_bump_slice();
        let markdown_links = markdown_links_builder.into_bump_slice();

        // --- Tags ---
        let mut tags_builder: BumpVec<'static, TagEntry<'static>> = BumpVec::new_in(arena_ref);
        if let Ok(scan_tags) = backend.scan_tags(text) {
            for t in scan_tags {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &t.name),
                });
            }
        }
        let tags = tags_builder.into_bump_slice();

        // --- Block IDs ---
        let mut blocks = HashMap::new();
        if let Ok(scan_blocks) = backend.scan_block_ids(text) {
            for b in scan_blocks {
                let id = arena_alloc_str(arena_ref, &b.id);
                let pos = byte_offset_to_position(&line_starts, b.offset);
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: Range::new(pos, pos),
                    },
                );
            }
        }

        // Build TOC and outline
        let toc = build_toc(arena_ref, headings);
        let outline = build_outline(arena_ref, headings);

        // XML tags: not supported by scan backend
        let xml_tags: &'static [XmlTagEntry<'static>] = &[];

        Self {
            _arena: Mutex::new(doc_arena),
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
    pub fn heading_by_slug(&self, slug: &str) -> Option<&HeadingEntry<'static>> {
        self.slug_to_heading
            .get(slug)
            .map(|&idx| &self.headings[idx])
    }

    /// Look up a block by its ID.
    pub fn block_by_id(&self, id: &str) -> Option<&BlockEntry<'static>> {
        self.blocks.get(id)
    }

    /// Get the flat table of contents.
    pub fn toc(&self) -> &[TocEntry<'static>] {
        self.toc
    }

    /// Get the outline tree.
    pub fn outline(&self) -> &OutlineNode<'static> {
        &self.outline
    }

    /// Get all indexed headings.
    pub fn headings(&self) -> &[HeadingEntry<'static>] {
        self.headings
    }

    /// Get all indexed wiki links.
    pub fn wiki_links(&self) -> &[WikiLinkEntry<'static>] {
        self.wiki_links
    }

    /// Get all indexed tags.
    pub fn tags(&self) -> &[TagEntry<'static>] {
        self.tags
    }

    /// Get all indexed markdown links.
    pub fn markdown_links(&self) -> &[MarkdownLinkEntry<'static>] {
        self.markdown_links
    }

    /// Get all indexed XML tags.
    pub fn xml_tags(&self) -> &[XmlTagEntry<'static>] {
        self.xml_tags
    }

    /// Get all block IDs in this document.
    pub fn block_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.blocks.keys().copied()
    }
}

impl fmt::Debug for DocumentIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentIndex")
            .field("headings", &self.headings.len())
            .field("blocks", &self.blocks.len())
            .field("toc", &self.toc.len())
            .field("wiki_links", &self.wiki_links.len())
            .field("tags", &self.tags.len())
            .field("markdown_links", &self.markdown_links.len())
            .field("xml_tags", &self.xml_tags.len())
            .finish()
    }
}

/// Build flat TOC entries with depth calculation.
fn build_toc<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> &'arena [TocEntry<'arena>] {
    let mut toc = BumpVec::new_in(arena);
    let mut level_stack: Vec<u8> = Vec::new();

    for h in headings {
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
            text: h.text,
            slug: h.slug,
            level: h.level,
            depth,
        });
    }

    toc.into_bump_slice()
}

#[derive(Debug, Clone)]
struct TempOutline<'arena> {
    heading: Option<HeadingEntry<'arena>>,
    children: Vec<TempOutline<'arena>>,
}

fn get_temp_node_mut<'tree, 'arena>(
    root: &'tree mut TempOutline<'arena>,
    path: &[usize],
) -> &'tree mut TempOutline<'arena> {
    let mut current = root;
    for &idx in path {
        current = &mut current.children[idx];
    }
    current
}

fn freeze_outline<'arena>(arena: &'arena Bump, node: TempOutline<'arena>) -> OutlineNode<'arena> {
    let mut children = BumpVec::new_in(arena);
    for child in node.children {
        children.push(freeze_outline(arena, child));
    }

    OutlineNode {
        heading: node.heading,
        children: children.into_bump_slice(),
    }
}

/// Build outline tree from heading entries.
fn build_outline<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> OutlineNode<'arena> {
    let mut root = TempOutline {
        heading: None,
        children: Vec::new(),
    };

    // Stack entries are (heading level, path of child indices from root).
    let mut stack: Vec<(u8, Vec<usize>)> = Vec::new();

    for h in headings {
        let node = TempOutline {
            heading: Some(h.clone()),
            children: Vec::new(),
        };

        while let Some((lvl, _)) = stack.last() {
            if *lvl >= h.level {
                stack.pop();
            } else {
                break;
            }
        }

        if stack.is_empty() {
            root.children.push(node);
            let idx = root.children.len() - 1;
            stack.push((h.level, vec![idx]));
        } else {
            let parent_path = stack.last().expect("stack not empty").1.clone();
            let parent = get_temp_node_mut(&mut root, &parent_path);
            parent.children.push(node);
            let child_idx = parent.children.len() - 1;

            let mut child_path = parent_path;
            child_path.push(child_idx);
            stack.push((h.level, child_path));
        }
    }

    freeze_outline(arena, root)
}

// ---------------------------------------------------------------------------
// Byte-offset to Position helpers (for scan-based construction)
// ---------------------------------------------------------------------------

/// Build a sorted list of byte offsets where each line starts.
/// Line 0 starts at offset 0. Line N starts after the N-th newline.
#[cfg(feature = "zig-kernels")]
fn byte_offset_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

/// Convert a byte offset to a Position (0-based line, 0-based character).
#[cfg(feature = "zig-kernels")]
fn byte_offset_to_position(line_starts: &[u32], offset: u32) -> Position {
    // Binary search for the line containing this offset
    let line = match line_starts.binary_search(&offset) {
        Ok(exact) => exact,        // offset is exactly at a line start
        Err(insert) => insert - 1, // offset is within the previous line
    };
    let col = offset - line_starts[line];
    Position::new(line as u32, col)
}

#[cfg(test)]
mod arena_allocation_tests {
    use super::*;
    use markymark_parser::Parser;

    fn build_index(source: &str) -> DocumentIndex {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse(source).unwrap();
        DocumentIndex::from_ast(ast)
    }

    #[test]
    fn heading_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = HeadingEntry {
            text: arena.alloc_str("Intro"),
            slug: arena.alloc_str("intro"),
            level: 1,
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
        };

        assert_eq!(entry.text, "Intro");
        assert_eq!(entry.slug, "intro");
        assert_eq!(entry.level, 1);
    }

    #[test]
    fn block_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = BlockEntry {
            id: arena.alloc_str("block-1"),
            range: Range::new(Position::new(0, 0), Position::new(0, 7)),
        };

        assert_eq!(entry.id, "block-1");
    }

    #[test]
    fn toc_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = TocEntry {
            text: arena.alloc_str("Section"),
            slug: arena.alloc_str("section"),
            level: 2,
            depth: 1,
        };

        assert_eq!(entry.text, "Section");
        assert_eq!(entry.slug, "section");
        assert_eq!(entry.depth, 1);
    }

    #[test]
    fn outline_node_uses_arena_lifetime() {
        let root = OutlineNode {
            heading: None,
            children: &[],
        };

        assert!(root.heading.is_none());
        assert!(root.children.is_empty());
    }

    #[test]
    fn wiki_link_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = WikiLinkEntry {
            target: arena.alloc_str("TargetPage"),
            alias: Some(arena.alloc_str("Alias")),
            heading: Some(arena.alloc_str("Section")),
            range: Range::new(Position::new(0, 0), Position::new(0, 10)),
        };

        assert_eq!(entry.target, "TargetPage");
        assert_eq!(entry.alias, Some("Alias"));
        assert_eq!(entry.heading, Some("Section"));
    }

    #[test]
    fn tag_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = TagEntry {
            name: arena.alloc_str("project/feature"),
        };

        assert_eq!(entry.name, "project/feature");
    }

    #[test]
    fn markdown_link_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let entry = MarkdownLinkEntry {
            text: arena.alloc_str("Example"),
            url: arena.alloc_str("https://example.com"),
            anchor: Some(arena.alloc_str("a")),
            range: Range::new(Position::new(0, 0), Position::new(0, 7)),
        };

        assert_eq!(entry.text, "Example");
        assert_eq!(entry.url, "https://example.com");
        assert_eq!(entry.anchor, Some("a"));
    }

    #[test]
    fn xml_tag_entry_uses_arena_lifetime() {
        let arena = Bump::new();
        let mut attrs = HashMap::new();
        let priority: &str = arena.alloc_str("priority");
        let high: &str = arena.alloc_str("high");
        attrs.insert(priority, high);

        let entry = XmlTagEntry {
            tag_name: arena.alloc_str("goal"),
            attributes: attrs,
            is_self_closing: false,
            is_unclosed: false,
            range: Range::new(Position::new(0, 0), Position::new(0, 6)),
        };

        assert_eq!(entry.tag_name, "goal");
        assert_eq!(entry.attributes.get("priority"), Some(&"high"));
    }

    #[test]
    fn document_index_uses_arena_lifetime() {
        let index = build_index("# Root\n\n## Child\n");

        assert_eq!(index.headings().len(), 2);
        assert_eq!(index.headings()[0].text, "Root");
        assert_eq!(index.headings()[1].slug, "child");
    }

    #[test]
    fn document_index_uses_hashbrown_with_arena() {
        let index = build_index("# Root\n\nA block ^block-1\n");

        // HashMap-backed lookups should work for arena-allocated keys.
        assert!(index.heading_by_slug("root").is_some());
        assert!(index.block_by_id("block-1").is_some());
    }

    #[test]
    fn xml_tag_entry_attributes_arena_map() {
        let index = build_index("<goal priority=\"high\" status=\"open\">Ship</goal>\n");

        let tags = index.xml_tags();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].attributes.get("priority"), Some(&"high"));
        assert_eq!(tags[0].attributes.get("status"), Some(&"open"));
    }

    #[test]
    fn document_index_vecs_become_slices() {
        let index = build_index("# A\n\n## B\n\n[[Page]]\n#tag\n");

        let _: &[HeadingEntry<'static>] = index.headings();
        let _: &[TocEntry<'static>] = index.toc();
        let _: &[WikiLinkEntry<'static>] = index.wiki_links();
        let _: &[TagEntry<'static>] = index.tags();
        let _: &[MarkdownLinkEntry<'static>] = index.markdown_links();
        let _: &[XmlTagEntry<'static>] = index.xml_tags();

        assert!(!index.headings().is_empty());
        assert!(!index.toc().is_empty());
    }

    #[test]
    fn outline_node_children_arena_slice() {
        let index = build_index("# Root\n\n## Child\n\n### Grandchild\n");

        let outline = index.outline();
        assert_eq!(outline.children.len(), 1);
        assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
        assert_eq!(outline.children[0].children.len(), 1);
    }

    #[test]
    fn from_ast_propagates_arena_lifetime() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("# Arena\n").unwrap();
        let index = DocumentIndex::from_ast(ast);

        let heading: &HeadingEntry<'static> = &index.headings()[0];
        assert_eq!(heading.text, "Arena");
    }

    #[test]
    fn heading_by_slug_returns_arena_ref() {
        let index = build_index("# Root\n\n## Root\n");

        let heading = index.heading_by_slug("root").unwrap();
        let _: &HeadingEntry<'static> = heading;
        assert_eq!(heading.text, "Root");
    }

    #[test]
    fn toc_returns_arena_slice() {
        let index = build_index("# A\n\n## B\n\n### C\n");

        let toc: &[TocEntry<'static>] = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 2);
    }

    #[test]
    fn parser_types_flow_to_index() {
        let index =
            build_index("# Heading\n\n[[Page#section]]\n#tag\n[Link](https://example.com#frag)\n");

        assert_eq!(index.headings()[0].text, "Heading");
        assert_eq!(index.wiki_links()[0].target, "Page");
        assert_eq!(index.wiki_links()[0].heading, Some("section"));

        // Tag extraction from this fixture can include heading anchors as tags;
        // assert that our expected tag is present rather than position-dependent.
        assert!(index.tags().iter().any(|t| t.name == "tag"));

        assert_eq!(index.markdown_links()[0].anchor, Some("frag"));
    }

    #[test]
    fn document_index_to_realm_integration() {
        let index_a = build_index("# Doc A\n\nA block ^a\n");
        let index_b = build_index("# Doc B\n\nA block ^b\n");

        assert_eq!(index_a.headings()[0].text, "Doc A");
        assert_eq!(index_b.headings()[0].text, "Doc B");
        assert!(index_a.block_by_id("a").is_some());
        assert!(index_b.block_by_id("b").is_some());
    }

    /// Test that take_arena properly returns a valid arena after consuming the AST.
    /// Exercises the safe arena transfer path that replaces ptr::read + mem::forget.
    #[test]
    fn take_arena_returns_valid_arena() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("---\ntitle: test\n---\n# Hello\n\n[[link]]\n")
            .unwrap();

        // take_arena should consume the AST and return a usable arena
        let arena = ast.take_arena();
        let bump = arena.bump();

        // Arena should still be valid — bump-allocated data survives transfer
        let s = bump.alloc_str("validation");
        assert_eq!(s, "validation");
    }

    /// Regression test for marky-4aa: mem::forget in from_ast leaks AST heap
    /// allocations (source String, root_elements Vec, md_tree MarkdownTree,
    /// Box<DocumentArena> shell). Under Miri, this test fails if any of those
    /// fields are leaked.
    ///
    /// Run with: `cargo +nightly miri test -p markymark-index from_ast_does_not_leak`
    #[test]
    fn from_ast_does_not_leak() {
        // Exercise the full extraction pipeline with rich content:
        // - frontmatter (triggers ArenaHashMap in parser types)
        // - headings, wiki links, tags, markdown links, block IDs
        let source = concat!(
            "---\n",
            "title: Leak Test\n",
            "tags:\n",
            "  - alpha\n",
            "  - beta\n",
            "---\n",
            "# Heading One\n",
            "\n",
            "Some text with a [[wiki-link]] and #tag.\n",
            "\n",
            "## Heading Two\n",
            "\n",
            "[regular link](https://example.com)\n",
            "\n",
            "Block content ^block-id\n",
        );
        let index = build_index(source);

        // Verify the index was built correctly before dropping
        assert_eq!(index.headings().len(), 2);
        assert!(!index.wiki_links().is_empty());
        assert!(!index.tags().is_empty());
        assert!(!index.markdown_links().is_empty());
        assert!(index.block_by_id("block-id").is_some());

        // Drop the index — under Miri, leaked allocations from mem::forget
        // will be reported as errors here.
        drop(index);
    }
}

// ---------------------------------------------------------------------------
// Scan-based construction tests (zig-kernels only)
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "zig-kernels"))]
mod scan_tests {
    use super::*;
    use markymark_core::scanner::ZigScanBackend;
    use markymark_parser::Parser;

    fn build_index_from_scan(source: &str) -> DocumentIndex {
        let backend = ZigScanBackend;
        DocumentIndex::from_scan(source, &backend)
    }

    fn build_index_from_ast(source: &str) -> DocumentIndex {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse(source).unwrap();
        DocumentIndex::from_ast(ast)
    }

    #[test]
    fn test_from_scan_empty_document() {
        let index = build_index_from_scan("");
        assert!(index.headings().is_empty());
        assert!(index.wiki_links().is_empty());
        assert!(index.tags().is_empty());
        assert!(index.markdown_links().is_empty());
        assert!(index.toc().is_empty());
    }

    #[test]
    fn test_from_scan_single_heading() {
        let index = build_index_from_scan("# Hello\n");
        assert_eq!(index.headings().len(), 1);
        assert_eq!(index.headings()[0].text, "Hello");
        assert_eq!(index.headings()[0].level, 1);
        assert_eq!(index.headings()[0].slug, "hello");
    }

    #[test]
    fn test_from_scan_multiple_headings() {
        let index = build_index_from_scan("# First\n\n## Second\n\n### Third\n");
        assert_eq!(index.headings().len(), 3);
        assert_eq!(index.headings()[0].level, 1);
        assert_eq!(index.headings()[1].level, 2);
        assert_eq!(index.headings()[2].level, 3);
        assert!(index.heading_by_slug("first").is_some());
        assert!(index.heading_by_slug("second").is_some());
    }

    #[test]
    fn test_from_scan_toc_builds() {
        let index = build_index_from_scan("# Root\n\n## Child\n\n### Grandchild\n");
        let toc = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 2);
    }

    #[test]
    fn test_from_scan_outline_builds() {
        let index = build_index_from_scan("# Root\n\n## Child\n");
        let outline = index.outline();
        assert_eq!(outline.children.len(), 1);
        assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
    }

    #[test]
    fn test_from_scan_markdown_links() {
        let index = build_index_from_scan("See [example](https://example.com) here\n");
        assert_eq!(index.markdown_links().len(), 1);
        assert_eq!(index.markdown_links()[0].text, "example");
        assert_eq!(index.markdown_links()[0].url, "https://example.com");
    }

    #[test]
    fn test_from_scan_wiki_links() {
        let index = build_index_from_scan("See [[My Page]] here\n");
        assert_eq!(index.wiki_links().len(), 1);
        assert_eq!(index.wiki_links()[0].target, "My Page");
    }

    #[test]
    fn test_from_scan_tags() {
        let index = build_index_from_scan("text #topic #project\n");
        assert!(index.tags().len() >= 2);
        assert!(index.tags().iter().any(|t| t.name == "topic"));
        assert!(index.tags().iter().any(|t| t.name == "project"));
    }

    #[test]
    fn test_from_scan_block_ids() {
        let index = build_index_from_scan("some content ^my-block\n");
        assert!(index.block_by_id("my-block").is_some());
    }

    #[test]
    fn test_from_scan_xml_tags_empty() {
        // from_scan does not extract XML tags (scan backend doesn't support them)
        let index = build_index_from_scan("<goal>Ship</goal>\n");
        assert!(index.xml_tags().is_empty());
    }

    #[test]
    fn test_from_ast_unchanged() {
        // Verify from_ast still works exactly as before
        let index = build_index_from_ast("# Heading\n\n[[Page]]\n#tag\n");
        assert_eq!(index.headings()[0].text, "Heading");
        assert!(!index.wiki_links().is_empty());
        assert!(index.tags().iter().any(|t| t.name == "tag"));
    }

    #[test]
    fn test_parity_headings() {
        let text = "# First\n\n## Second\n\n### Third\n";
        let ast_idx = build_index_from_ast(text);
        let scan_idx = build_index_from_scan(text);

        assert_eq!(ast_idx.headings().len(), scan_idx.headings().len());
        for (a, s) in ast_idx.headings().iter().zip(scan_idx.headings().iter()) {
            assert_eq!(a.text, s.text);
            assert_eq!(a.level, s.level);
            assert_eq!(a.slug, s.slug);
        }
    }
}
