//! Document indexing: heading lookup, block lookup, TOC, outline tree.

mod helpers;
mod types;

mod from_ast;

#[cfg(feature = "zig-kernels")]
mod from_blob;

#[cfg(test)]
mod tests;

pub use helpers::slugify;
pub use types::*;

#[cfg(feature = "zig-kernels")]
pub use from_blob::extract_xml_tags_from_text;
#[cfg(feature = "zig-kernels")]
pub use from_blob::BlobError;

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::prelude::*;
use self_cell::self_cell;
use std::collections::HashMap as StdHashMap;
use std::fmt;

#[cfg(feature = "zig-kernels")]
use markymark_core::scanner::{ScanBackend, ScanLinkType};

/// Index of a single parsed markdown document.
///
/// Built from a [`markymark_parser::Ast`], provides fast lookups for
/// headings (by slug), block IDs, table of contents, and outline tree.
///
#[derive(Debug)]
struct DocumentOwner {
    arena: DocumentArena,
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
    code_spans: &'a [CodeSpanEntry<'a>],
    frontmatter: &'a [FrontmatterEntry<'a>],
    aliases: &'a [&'a str],
    properties: &'a [PropertyEntry<'a>],
    block_refs: &'a [BlockRefEntry<'a>],
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
/// # `Sync` implementation
///
/// `Bump: !Sync` (due to internal `Cell`) makes `DocumentArena: !Sync` and
/// therefore `DocumentIndexCell: !Sync` via auto-trait propagation. However,
/// tower-lsp requires `Send + Sync` for `RwLock<ServerState>`.
///
/// We provide `unsafe impl Sync` because the arena is only mutated during
/// the `self_cell` builder closure (single-threaded construction). After
/// construction, all access is read-only via `borrow_dependent()`. No public
/// API exposes `&Bump` or `&DocumentArena`. See the safety comment on the
/// `Sync` impl below.
///
/// **Invariant**: Do NOT add post-construction mutation of the arena.
pub struct DocumentIndex {
    cell: DocumentIndexCell,
}

// SAFETY: `DocumentArena` wraps `bumpalo::Bump` which is `Send + !Sync`.
// `Bump` is `!Sync` because its allocation pointer uses `Cell` (interior
// mutability). However, after `DocumentIndexCell::new()` completes:
//
// 1. No code path calls `Bump::alloc()` — all allocations happen inside the
//    builder closure during construction (single-threaded, synchronous).
// 2. All public accessors use `borrow_dependent()` returning immutable
//    references to arena-backed slices (`&[HeadingEntry]`, `&str`, etc.).
// 3. `DocumentOwner` is private — no external code can reach `&Bump`.
// 4. `borrow_owner()` is never called outside construction code.
//
// Therefore sharing `&DocumentIndex` across threads is safe: the `Cell`
// inside `Bump` is effectively frozen after construction.
// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
unsafe impl Sync for DocumentIndex {}

impl DocumentIndex {
    /// Build a document index from a scan backend (Zig SIMD path).
    ///
    /// Uses byte-offset based scanning instead of AST parsing. The scan backend
    /// provides heading, link, tag, and block-id extraction via SIMD kernels.
    /// XML tags are not supported by the scan path (returns empty slice).
    #[cfg(feature = "zig-kernels")]
    pub fn from_scan(text: &str, backend: &dyn ScanBackend) -> Self {
        // Pre-compute line starts for byte-offset → Position conversion
        let line_starts = helpers::byte_offset_line_starts(text);

        // Collect owned data from scan backend before entering self_cell closure.
        // Fall back to independent scans if scan_all fails so that headings
        // and links are never both silently dropped due to one-sided error.
        let (scan_headings, scan_links, scan_code_spans) = match backend.scan_all(text) {
            Ok(result) => (result.headings, result.links, result.code_spans),
            Err(_) => (
                backend.scan_headings(text).unwrap_or_default(),
                backend.scan_links(text).unwrap_or_default(),
                backend.scan_code_spans(text).unwrap_or_default(),
            ),
        };
        let scan_tags = backend.scan_tags(text).unwrap_or_default();
        let scan_blocks = backend.scan_block_ids(text).unwrap_or_default();

        let owner = DocumentOwner {
            arena: DocumentArena::new(),
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();

            for h in scan_headings {
                let base_slug = slugify(&h.text);
                let slug_owned = helpers::dedup_slug(&base_slug, &mut slug_counts);
                let heading_text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let pos = helpers::byte_offset_to_position(&line_starts, h.offset);
                let end_pos = helpers::byte_offset_to_position(
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
                let pos = helpers::byte_offset_to_position(&line_starts, l.offset);
                let end_offset = match l.link_type {
                    ScanLinkType::Markdown => {
                        l.offset + l.text.len() as u32 + l.target.len() as u32 + 4
                    }
                    ScanLinkType::Wiki if l.text != l.target => {
                        l.offset + l.target.len() as u32 + 1 + l.text.len() as u32 + 4
                    }
                    ScanLinkType::Wiki => l.offset + l.target.len() as u32 + 4,
                };
                let end_pos = helpers::byte_offset_to_position(&line_starts, end_offset);
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
                            start_byte: l.offset as usize,
                            end_byte: end_offset as usize,
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
                            start_byte: l.offset as usize,
                            end_byte: end_offset as usize,
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
                let pos = helpers::byte_offset_to_position(&line_starts, b.offset);
                let end_pos = helpers::byte_offset_to_position(
                    &line_starts,
                    b.offset + 1 + b.id.len() as u32,
                );
                let start_byte = b.offset as usize;
                let end_byte = (b.offset + 1 + b.id.len() as u32) as usize;
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: Range::new(pos, end_pos),
                        start_byte,
                        end_byte,
                    },
                );
            }

            // Build TOC and outline from headings
            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            // XML tags: not supported by scan backend
            let xml_tags = BumpVec::<XmlTagEntry<'_>>::new_in(arena_ref).into_bump_slice();

            // --- Code spans ---
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for cs in scan_code_spans {
                let text = arena_alloc_str(arena_ref, &cs.text);
                let pos = helpers::byte_offset_to_position(&line_starts, cs.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, cs.end_offset);
                cs_builder.push(CodeSpanEntry {
                    text,
                    range: Range::new(pos, end_pos),
                    start_byte: cs.offset as usize,
                    end_byte: cs.end_offset as usize,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            // Frontmatter/properties/block-refs: not available from scan backend
            let frontmatter = BumpVec::<FrontmatterEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let aliases = BumpVec::<&str>::new_in(arena_ref).into_bump_slice();
            let properties = BumpVec::<PropertyEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let block_refs = BumpVec::<BlockRefEntry<'_>>::new_in(arena_ref).into_bump_slice();

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
                code_spans,
                frontmatter,
                aliases,
                properties,
                block_refs,
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

    /// Get all inline code span entries.
    pub fn code_spans<'a>(&'a self) -> &'a [CodeSpanEntry<'a>] {
        self.cell.borrow_dependent().code_spans
    }

    /// Get all block IDs in this document.
    pub fn block_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.cell.borrow_dependent().blocks.keys().copied()
    }

    /// Get all frontmatter entries for this document.
    pub fn frontmatter<'a>(&'a self) -> &'a [FrontmatterEntry<'a>] {
        self.cell.borrow_dependent().frontmatter
    }

    /// Get Obsidian aliases from the frontmatter `aliases` field.
    pub fn aliases(&self) -> &[&str] {
        self.cell.borrow_dependent().aliases
    }

    /// Get all Logseq inline property entries for this document.
    pub fn properties<'a>(&'a self) -> &'a [PropertyEntry<'a>] {
        self.cell.borrow_dependent().properties
    }

    /// Get all Logseq block references (`((uuid))`) in this document.
    pub fn block_refs<'a>(&'a self) -> &'a [BlockRefEntry<'a>] {
        self.cell.borrow_dependent().block_refs
    }
}

impl fmt::Debug for DocumentIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dep = self.cell.borrow_dependent();
        f.debug_struct("DocumentIndex")
            .field("headings", &dep.headings.len())
            .field("blocks", &dep.blocks.len())
            .field("toc", &dep.toc.len())
            .field("outline", &dep.outline.children.len())
            .field("wiki_links", &dep.wiki_links.len())
            .field("tags", &dep.tags.len())
            .field("markdown_links", &dep.markdown_links.len())
            .field("xml_tags", &dep.xml_tags.len())
            .field("frontmatter", &dep.frontmatter.len())
            .field("aliases", &dep.aliases.len())
            .field("properties", &dep.properties.len())
            .field("block_refs", &dep.block_refs.len())
            .finish()
    }
}
