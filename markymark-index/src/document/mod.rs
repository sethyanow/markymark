//! Document indexing: heading lookup, block lookup, TOC, outline tree.

mod helpers;
mod types;

mod from_ast;

#[cfg(feature = "zig-kernels")]
mod from_blob;

#[cfg(feature = "zig-kernels")]
mod from_scan;

#[cfg(test)]
mod tests;

pub use helpers::slugify;
pub use types::*;

#[cfg(feature = "zig-kernels")]
pub use from_blob::extract_xml_tags_from_text;
#[cfg(feature = "zig-kernels")]
pub use from_blob::BlobError;

use hashbrown::HashMap;
use markymark_core::arena::DocumentArena;
use self_cell::self_cell;
use std::fmt;

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
