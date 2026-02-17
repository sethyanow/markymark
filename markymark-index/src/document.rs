//! Document indexing: heading lookup, block lookup, TOC, outline tree.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::prelude::*;
use markymark_parser::Ast;
use self_cell::self_cell;
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
#[derive(Debug)]
struct DocumentOwner {
    arena: Mutex<DocumentArena>,
}

#[derive(Debug)]
struct DocumentDependent<'a> {
    headings: &'a [HeadingEntry<'a>],
    slug_to_heading: HashMap<&'a str, usize>,
    blocks: HashMap<&'a str, BlockEntry<'a>>,
    toc: &'a [TocEntry<'a>],
    outline: OutlineNode<'a>,
    wiki_links: &'a [WikiLinkEntry<'a>],
    tags: &'a [TagEntry<'a>],
    markdown_links: &'a [MarkdownLinkEntry<'a>],
    xml_tags: &'a [XmlTagEntry<'a>],
}

self_cell!(
    struct DocumentIndexCell {
        owner: DocumentOwner,

        #[covariant]
        dependent: DocumentDependent,
    }

    impl { Debug }
);

/// # Safety (self-referential arena pattern)
///
/// `DocumentIndex` stores arena-backed references in a `self_cell` dependent
/// tied to an owned `DocumentArena`. Public accessors return references bound
/// to `&self`, preventing lifetime escape in safe code.
///
/// # Why `Mutex<DocumentArena>`
///
/// `Bump: !Sync` makes `DocumentArena: !Sync`, which prevents `DocumentIndex`
/// from implementing `Send + Sync`. tower-lsp requires `Send + 'static` for
/// async handlers that store state in `RwLock<ServerState>`. Wrapping the arena
/// in `Mutex` preserves `Send + Sync` compatibility while retaining arena-backed
/// allocation behavior.
pub struct DocumentIndex {
    cell: DocumentIndexCell,
}

impl DocumentIndex {
    #[inline]
    fn arena_ref(owner: &DocumentOwner) -> &Bump {
        let arena_guard = owner
            .arena
            .lock()
            .expect("DocumentIndex arena mutex should not be poisoned");
        let arena_ptr: *const DocumentArena = &*arena_guard as *const DocumentArena;
        drop(arena_guard);

        // SAFETY: `arena_ptr` points to the `DocumentArena` stored inside the
        // owner mutex. The owner outlives all dependent borrows and we never
        // mutate or move the arena after construction.
        unsafe { (*arena_ptr).bump() }
    }

    /// Build a document index from a parsed AST.
    ///
    /// Extracts owned intermediate records, moves the parser arena into this
    /// index, and allocates the final index entries in one arena-backed pass.
    pub fn from_ast(ast: Ast) -> Self {
        #[derive(Debug)]
        struct HeadingOwned {
            text: String,
            level: u8,
            range: Range,
        }
        #[derive(Debug)]
        struct BlockOwned {
            id: String,
            range: Range,
        }
        #[derive(Debug)]
        struct WikiLinkOwned {
            target: String,
            alias: Option<String>,
            heading: Option<String>,
            range: Range,
        }
        #[derive(Debug)]
        struct TagOwned {
            name: String,
        }
        #[derive(Debug)]
        struct MarkdownLinkOwned {
            text: String,
            url: String,
            anchor: Option<String>,
            range: Range,
        }
        #[derive(Debug)]
        struct XmlTagOwned {
            tag_name: String,
            attributes: Vec<(String, String)>,
            is_self_closing: bool,
            is_unclosed: bool,
            range: Range,
        }

        let mut headings_owned = Vec::new();
        for element in ast.root_elements() {
            if let Some(h) = element.as_heading() {
                headings_owned.push(HeadingOwned {
                    text: h.text().to_string(),
                    level: h.level(),
                    range: h.range(),
                });
            }
        }

        let mut blocks_owned = Vec::new();
        for block_id in ast.extract_block_ids() {
            blocks_owned.push(BlockOwned {
                id: block_id.id().to_string(),
                range: block_id.range(),
            });
        }

        let mut wiki_links_owned = Vec::new();
        for wl in ast.extract_wiki_links() {
            if wl.target_page().is_none()
                && wl.target_heading().is_none()
                && wl.target_block_id().is_none()
            {
                continue;
            }

            wiki_links_owned.push(WikiLinkOwned {
                target: wl.target_page().unwrap_or("").to_string(),
                alias: wl.alias().map(str::to_string),
                heading: wl.target_heading().map(str::to_string),
                range: wl.range(),
            });
        }

        let mut tags_owned = Vec::new();
        for tag in ast.extract_tags() {
            tags_owned.push(TagOwned {
                name: tag.name().to_string(),
            });
        }

        let mut markdown_links_owned = Vec::new();
        for ml in ast.extract_markdown_links() {
            markdown_links_owned.push(MarkdownLinkOwned {
                text: ml.text().to_string(),
                url: ml.url().to_string(),
                anchor: ml.anchor().map(str::to_string),
                range: ml.range(),
            });
        }

        let mut xml_tags_owned = Vec::new();
        for xt in ast.extract_xml_tags() {
            let attributes = xt
                .attributes()
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect::<Vec<_>>();
            xml_tags_owned.push(XmlTagOwned {
                tag_name: xt.tag_name().to_string(),
                attributes,
                is_self_closing: xt.is_self_closing(),
                is_unclosed: xt.is_unclosed(),
                range: xt.range(),
            });
        }

        let owner = DocumentOwner {
            arena: Mutex::new(ast.into_arena()),
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = Self::arena_ref(owner);

            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();
            for h in headings_owned {
                let base_slug = slugify(&h.text);
                let slug_owned = dedup_slug(&base_slug, &mut slug_counts);
                let text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level,
                    range: h.range,
                });
            }
            let headings = headings_builder.into_bump_slice();

            let mut blocks = HashMap::new();
            for block in blocks_owned {
                let id = arena_alloc_str(arena_ref, &block.id);
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: block.range,
                    },
                );
            }

            let toc = build_toc(arena_ref, headings);
            let outline = build_outline(arena_ref, headings);

            let mut wiki_links_builder = BumpVec::new_in(arena_ref);
            for wl in wiki_links_owned {
                wiki_links_builder.push(WikiLinkEntry {
                    target: arena_alloc_str(arena_ref, &wl.target),
                    alias: wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a)),
                    heading: wl.heading.as_deref().map(|h| arena_alloc_str(arena_ref, h)),
                    range: wl.range,
                });
            }
            let wiki_links = wiki_links_builder.into_bump_slice();

            let mut tags_builder = BumpVec::new_in(arena_ref);
            for tag in tags_owned {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &tag.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            let mut markdown_links_builder = BumpVec::new_in(arena_ref);
            for ml in markdown_links_owned {
                markdown_links_builder.push(MarkdownLinkEntry {
                    text: arena_alloc_str(arena_ref, &ml.text),
                    url: arena_alloc_str(arena_ref, &ml.url),
                    anchor: ml.anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a)),
                    range: ml.range,
                });
            }
            let markdown_links = markdown_links_builder.into_bump_slice();

            let mut xml_tags_builder = BumpVec::new_in(arena_ref);
            for xt in xml_tags_owned {
                let mut attributes = HashMap::new();
                for (k, v) in xt.attributes {
                    let k_ref = arena_alloc_str(arena_ref, &k);
                    let v_ref = arena_alloc_str(arena_ref, &v);
                    attributes.insert(k_ref, v_ref);
                }
                xml_tags_builder.push(XmlTagEntry {
                    tag_name: arena_alloc_str(arena_ref, &xt.tag_name),
                    attributes,
                    is_self_closing: xt.is_self_closing,
                    is_unclosed: xt.is_unclosed,
                    range: xt.range,
                });
            }
            let xml_tags = xml_tags_builder.into_bump_slice();

            DocumentDependent {
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
        });

        Self { cell }
    }

    /// Build a document index from a scan backend (Zig SIMD path).
    ///
    /// Uses byte-offset based scanning instead of AST parsing. The scan backend
    /// provides heading, link, tag, and block-id extraction via SIMD kernels.
    /// XML tags are not supported by the scan path (returns empty slice).
    #[cfg(feature = "zig-kernels")]
    pub fn from_scan(text: &str, backend: &dyn ScanBackend) -> Self {
        // Pre-compute line starts for byte-offset → Position conversion
        let line_starts = byte_offset_line_starts(text);

        // Collect owned data from scan backend before entering self_cell closure
        let scan_headings = backend.scan_headings(text).unwrap_or_default();
        let scan_links = backend.scan_links(text).unwrap_or_default();
        let scan_tags = backend.scan_tags(text).unwrap_or_default();
        let scan_blocks = backend.scan_block_ids(text).unwrap_or_default();

        let owner = DocumentOwner {
            arena: Mutex::new(DocumentArena::new()),
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = Self::arena_ref(owner);

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();

            for h in scan_headings {
                let base_slug = slugify(&h.text);
                let slug_owned = dedup_slug(&base_slug, &mut slug_counts);
                let heading_text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let pos = byte_offset_to_position(&line_starts, h.offset);
                let end_pos = byte_offset_to_position(
                    &line_starts,
                    h.offset + h.level as u32 + 1 + h.text.len() as u32,
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
            let headings = headings_builder.into_bump_slice();

            // --- Links (split into wiki and markdown) ---
            let mut wiki_links_builder = BumpVec::new_in(arena_ref);
            let mut markdown_links_builder = BumpVec::new_in(arena_ref);

            for l in scan_links {
                let pos = byte_offset_to_position(&line_starts, l.offset);
                let end_offset = match l.link_type {
                    ScanLinkType::Markdown => {
                        l.offset + l.text.len() as u32 + l.target.len() as u32 + 4
                    }
                    ScanLinkType::Wiki if l.text != l.target => {
                        l.offset + l.target.len() as u32 + 1 + l.text.len() as u32 + 4
                    }
                    ScanLinkType::Wiki => l.offset + l.target.len() as u32 + 4,
                };
                let end_pos = byte_offset_to_position(&line_starts, end_offset);
                let range = Range::new(pos, end_pos);

                match l.link_type {
                    ScanLinkType::Wiki => {
                        let target = arena_alloc_str(arena_ref, &l.target);
                        let alias = if l.text != l.target {
                            Some(arena_alloc_str(arena_ref, &l.text))
                        } else {
                            None
                        };
                        wiki_links_builder.push(WikiLinkEntry {
                            target,
                            alias,
                            heading: None,
                            range,
                        });
                    }
                    ScanLinkType::Markdown => {
                        let link_text = arena_alloc_str(arena_ref, &l.text);
                        let (url_str, anchor) = if let Some(hash_pos) = l.target.find('#') {
                            (&l.target[..hash_pos], Some(&l.target[hash_pos + 1..]))
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
            let wiki_links = wiki_links_builder.into_bump_slice();
            let markdown_links = markdown_links_builder.into_bump_slice();

            // --- Tags ---
            let mut tags_builder = BumpVec::new_in(arena_ref);
            for t in scan_tags {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &t.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs ---
            let mut blocks = HashMap::new();
            for b in scan_blocks {
                let id = arena_alloc_str(arena_ref, &b.id);
                let pos = byte_offset_to_position(&line_starts, b.offset);
                let end_pos =
                    byte_offset_to_position(&line_starts, b.offset + 1 + b.id.len() as u32);
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: Range::new(pos, end_pos),
                    },
                );
            }

            // Build TOC and outline from headings
            let toc = build_toc(arena_ref, headings);
            let outline = build_outline(arena_ref, headings);

            // XML tags: not supported by scan backend
            let xml_tags = BumpVec::<XmlTagEntry<'_>>::new_in(arena_ref).into_bump_slice();

            DocumentDependent {
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
        });

        Self { cell }
    }

    /// Look up a heading by its slug.
    pub fn heading_by_slug<'a>(&'a self, slug: &str) -> Option<&'a HeadingEntry<'a>> {
        let dep = self.cell.borrow_dependent();
        dep.slug_to_heading.get(slug).map(|&idx| &dep.headings[idx])
    }

    /// Look up a block by its ID.
    pub fn block_by_id<'a>(&'a self, id: &str) -> Option<&'a BlockEntry<'a>> {
        self.cell.borrow_dependent().blocks.get(id)
    }

    /// Get the flat table of contents.
    pub fn toc<'a>(&'a self) -> &'a [TocEntry<'a>] {
        self.cell.borrow_dependent().toc
    }

    /// Get the outline tree.
    pub fn outline<'a>(&'a self) -> &'a OutlineNode<'a> {
        &self.cell.borrow_dependent().outline
    }

    /// Get all indexed headings.
    ///
    /// ```compile_fail
    /// use markymark_index::DocumentIndex;
    /// use markymark_parser::Parser;
    ///
    /// fn leak_index_text() -> &'static str {
    ///     let mut parser = Parser::new().unwrap();
    ///     let ast = parser.parse("# Title").unwrap();
    ///     let index = DocumentIndex::from_ast(ast);
    ///     index.headings()[0].text
    /// }
    /// ```
    pub fn headings<'a>(&'a self) -> &'a [HeadingEntry<'a>] {
        self.cell.borrow_dependent().headings
    }

    /// Get all indexed wiki links.
    pub fn wiki_links<'a>(&'a self) -> &'a [WikiLinkEntry<'a>] {
        self.cell.borrow_dependent().wiki_links
    }

    /// Get all indexed tags.
    pub fn tags<'a>(&'a self) -> &'a [TagEntry<'a>] {
        self.cell.borrow_dependent().tags
    }

    /// Get all indexed markdown links.
    pub fn markdown_links<'a>(&'a self) -> &'a [MarkdownLinkEntry<'a>] {
        self.cell.borrow_dependent().markdown_links
    }

    /// Get all indexed XML tags.
    pub fn xml_tags<'a>(&'a self) -> &'a [XmlTagEntry<'a>] {
        self.cell.borrow_dependent().xml_tags
    }

    /// Get all block IDs in this document.
    pub fn block_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.cell.borrow_dependent().blocks.keys().copied()
    }
}

impl fmt::Debug for DocumentIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dep = self.cell.borrow_dependent();
        f.debug_struct("DocumentIndex")
            .field("headings", &dep.headings.len())
            .field("blocks", &dep.blocks.len())
            .field("toc", &dep.toc.len())
            .field("wiki_links", &dep.wiki_links.len())
            .field("tags", &dep.tags.len())
            .field("markdown_links", &dep.markdown_links.len())
            .field("xml_tags", &dep.xml_tags.len())
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
    let line = match line_starts.binary_search(&offset) {
        Ok(exact) => exact,
        Err(insert) => insert - 1,
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

        let _: &[HeadingEntry<'_>] = index.headings();
        let _: &[TocEntry<'_>] = index.toc();
        let _: &[WikiLinkEntry<'_>] = index.wiki_links();
        let _: &[TagEntry<'_>] = index.tags();
        let _: &[MarkdownLinkEntry<'_>] = index.markdown_links();
        let _: &[XmlTagEntry<'_>] = index.xml_tags();

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

        let heading: &HeadingEntry<'_> = &index.headings()[0];
        assert_eq!(heading.text, "Arena");
    }

    #[test]
    fn heading_by_slug_returns_arena_ref() {
        let index = build_index("# Root\n\n## Root\n");

        let heading = index.heading_by_slug("root").unwrap();
        let _: &HeadingEntry<'_> = heading;
        assert_eq!(heading.text, "Root");
    }

    #[test]
    fn toc_returns_arena_slice() {
        let index = build_index("# A\n\n## B\n\n### C\n");

        let toc: &[TocEntry<'_>] = index.toc();
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
}

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
        let index = build_index_from_scan("<goal>Ship</goal>\n");
        assert!(index.xml_tags().is_empty());
    }

    #[test]
    fn test_from_ast_unchanged() {
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

    // --- Bug fix tests: wiki link range calculation (marky-x3x #1) ---

    #[test]
    fn test_from_scan_wiki_link_range_no_alias() {
        let index = build_index_from_scan("See [[My Page]] here\n");
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "My Page");
        assert_eq!(wl.range.start, Position::new(0, 4));
        assert_eq!(wl.range.end, Position::new(0, 15));
    }

    #[test]
    fn test_from_scan_wiki_link_range_with_alias() {
        let index = build_index_from_scan("See [[target|display]] here\n");
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "target");
        assert!(wl.alias.is_some());
        assert_eq!(wl.alias.unwrap(), "display");
        assert_eq!(wl.range.start, Position::new(0, 4));
        assert_eq!(wl.range.end, Position::new(0, 22));
    }

    #[test]
    fn test_from_scan_markdown_link_range() {
        let index = build_index_from_scan("See [example](https://example.com) here\n");
        let ml = &index.markdown_links()[0];
        assert_eq!(ml.text, "example");
        assert_eq!(ml.range.start, Position::new(0, 4));
        assert_eq!(ml.range.end, Position::new(0, 34));
    }

    // --- Bug fix test: block ID range (marky-x3x #2) ---

    #[test]
    fn test_from_scan_block_id_range_nonzero_width() {
        let index = build_index_from_scan("some content ^my-block\n");
        let block = index.block_by_id("my-block").unwrap();
        assert_eq!(block.range.start, Position::new(0, 13));
        assert_eq!(block.range.end, Position::new(0, 22));
        assert_ne!(block.range.start, block.range.end);
    }
}
