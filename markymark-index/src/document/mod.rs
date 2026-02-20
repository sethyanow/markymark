//! Document indexing: heading lookup, block lookup, TOC, outline tree.

mod helpers;
mod types;

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

        // SAFETY:
        // 1) `DocumentOwner.arena` is initialized exactly once before
        //    `DocumentIndexCell::try_new` and remains owned by the cell owner.
        // 2) The `DocumentArena` is never moved and is treated as immutable
        //    after construction; dependent values only borrow from it.
        // 3) `arena_ref` is only used during `from_ast` construction while
        //    building the dependent. It must not be used after construction.
        unsafe { (*arena_ptr).bump() }
    }

    /// Build a document index from a parsed AST.
    ///
    /// Extracts owned intermediate records, moves the parser arena into this
    /// index, and allocates the final index entries in one arena-backed pass.
    pub fn from_ast(ast: Ast) -> Self {
        Self::from_ast_with_overrides_opt(ast, IncrementalOverrides::default())
    }

    /// Build a document index from a parsed AST while overriding wiki-links.
    ///
    /// This is used by incremental reindexing paths that already computed
    /// a selective wiki-link merge and want to avoid full re-extraction.
    pub fn from_ast_with_wiki_links(ast: Ast, wiki_links: Vec<WikiLinkOwned>) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                wiki_links: Some(wiki_links),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST while overriding blocks.
    ///
    /// This is used by incremental reindexing paths that already computed
    /// a selective block merge and want to avoid full re-extraction.
    pub fn from_ast_with_blocks(ast: Ast, blocks: Vec<BlockOwned>) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                blocks: Some(blocks),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST while overriding both wiki-links and blocks.
    ///
    /// This is the primary incremental path when both extractors have been merged.
    pub fn from_ast_with_wiki_links_and_blocks(
        ast: Ast,
        wiki_links: Vec<WikiLinkOwned>,
        blocks: Vec<BlockOwned>,
    ) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                wiki_links: Some(wiki_links),
                blocks: Some(blocks),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST with selective extractor overrides.
    ///
    /// This is the primary construction path used by incremental reindexing. For each
    /// extractor, a `Some` override skips re-extraction and uses the provided data;
    /// `None` extracts fresh from the AST. Always use [`IncrementalOverrides`] rather
    /// than calling the convenience functions when multiple extractors need overrides.
    pub fn from_ast_with_overrides_opt(ast: Ast, overrides: IncrementalOverrides) -> Self {
        #[derive(Debug)]
        struct HeadingOwned {
            text: String,
            level: u8,
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

        let blocks_owned = if let Some(blocks_override) = overrides.blocks {
            blocks_override
        } else {
            let mut blocks_owned = Vec::new();
            for block_id in ast.extract_block_ids() {
                blocks_owned.push(BlockOwned {
                    id: block_id.id().to_string(),
                    range: block_id.range(),
                    start_byte: block_id.start_byte(),
                    end_byte: block_id.end_byte(),
                });
            }
            blocks_owned
        };

        let wiki_links_owned = if let Some(wiki_links_override) = overrides.wiki_links {
            wiki_links_override
        } else {
            let mut wiki_links_owned = Vec::new();
            for wl in ast.extract_wiki_links() {
                if wl.target_page().is_none()
                    && wl.target_heading().is_none()
                    && wl.target_block_id().is_none()
                {
                    continue;
                }

                let (start_byte, end_byte) = wl.byte_range();
                wiki_links_owned.push(WikiLinkOwned {
                    target: wl.target_page().unwrap_or("").to_string(),
                    alias: wl.alias().map(str::to_string),
                    heading: wl.target_heading().map(str::to_string),
                    range: wl.range(),
                    start_byte,
                    end_byte,
                });
            }
            wiki_links_owned
        };

        // Tags have no source range in the parser — always re-extract.
        // The `overrides.tags` field is present for API completeness but is always `None`.
        let tags_owned: Vec<TagOwned> = ast
            .extract_tags()
            .into_iter()
            .map(|tag| TagOwned {
                name: tag.name().to_string(),
            })
            .collect();

        let markdown_links_owned = if let Some(ml_override) = overrides.markdown_links {
            ml_override
        } else {
            ast.extract_markdown_links()
                .into_iter()
                .map(|ml| {
                    let (start_byte, end_byte) = ml.byte_range();
                    MarkdownLinkOwned {
                        text: ml.text().to_string(),
                        url: ml.url().to_string(),
                        anchor: ml.anchor().map(str::to_string),
                        range: ml.range(),
                        start_byte,
                        end_byte,
                    }
                })
                .collect()
        };

        let xml_tags_owned = if let Some(xt_override) = overrides.xml_tags {
            xt_override
        } else {
            ast.extract_xml_tags()
                .into_iter()
                .map(|xt| {
                    let mut attributes: Vec<(String, String)> = xt
                        .attributes()
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                        .collect();
                    attributes.sort_by(|a, b| a.0.cmp(&b.0));
                    let (start_byte, end_byte) = xt.byte_range();
                    XmlTagOwned {
                        tag_name: xt.tag_name().to_string(),
                        attributes,
                        is_self_closing: xt.is_self_closing(),
                        is_unclosed: xt.is_unclosed(),
                        range: xt.range(),
                        start_byte,
                        end_byte,
                    }
                })
                .collect()
        };

        // Extract frontmatter and properties as owned data BEFORE arena move.
        #[derive(Debug)]
        enum FrontmatterValueOwned {
            String(String),
            List(Vec<String>),
        }
        #[derive(Debug)]
        struct FrontmatterOwned {
            key: String,
            value: FrontmatterValueOwned,
        }
        #[derive(Debug)]
        enum PropertyValueOwned {
            String(String),
            List(Vec<String>),
            PageRef(String),
        }
        #[derive(Debug)]
        struct PropertyOwned {
            key: String,
            value: PropertyValueOwned,
        }

        let mut frontmatter_owned: Vec<FrontmatterOwned> = Vec::new();
        let mut aliases_owned: Vec<String> = Vec::new();

        if let Some(fm) = ast.frontmatter() {
            use markymark_parser::FrontmatterValue;
            for (key, value) in fm.iter() {
                let key_str = (*key).to_string();
                let value_owned = match value {
                    FrontmatterValue::String(s) => FrontmatterValueOwned::String((*s).to_string()),
                    FrontmatterValue::List(items) => {
                        FrontmatterValueOwned::List(items.iter().map(|s| s.to_string()).collect())
                    }
                };
                // Extract aliases separately for the dedicated accessor.
                if key_str == "aliases" {
                    match &value_owned {
                        FrontmatterValueOwned::String(s) => {
                            if !s.is_empty() {
                                aliases_owned.push(s.clone());
                            }
                        }
                        FrontmatterValueOwned::List(items) => {
                            aliases_owned.extend(items.iter().cloned());
                        }
                    }
                }
                frontmatter_owned.push(FrontmatterOwned {
                    key: key_str,
                    value: value_owned,
                });
            }
        }

        let mut properties_owned: Vec<PropertyOwned> = Vec::new();
        if let Some(props) = ast.page_properties() {
            use markymark_parser::PropertyValue;
            for (key, value) in props.iter() {
                let key_str = (*key).to_string();
                let value_owned = match value {
                    PropertyValue::String(s) => PropertyValueOwned::String((*s).to_string()),
                    PropertyValue::List(items) => {
                        PropertyValueOwned::List(items.iter().map(|s| s.to_string()).collect())
                    }
                    PropertyValue::PageRef(s) => PropertyValueOwned::PageRef((*s).to_string()),
                };
                properties_owned.push(PropertyOwned {
                    key: key_str,
                    value: value_owned,
                });
            }
        }

        // Extract block refs as owned data BEFORE arena move.
        #[derive(Debug)]
        struct BlockRefOwned {
            uuid: String,
            range: markymark_core::Range,
        }
        let block_refs_owned: Vec<BlockRefOwned> = ast
            .extract_block_refs()
            .into_iter()
            .map(|r| BlockRefOwned {
                uuid: r.uuid().to_string(),
                range: r.range(),
            })
            .collect();

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
                let slug_owned = helpers::dedup_slug(&base_slug, &mut slug_counts);
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
                        start_byte: block.start_byte,
                        end_byte: block.end_byte,
                    },
                );
            }

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            let mut wiki_links_builder = BumpVec::new_in(arena_ref);
            for wl in &wiki_links_owned {
                wiki_links_builder.push(WikiLinkEntry {
                    target: arena_alloc_str(arena_ref, &wl.target),
                    alias: wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a)),
                    heading: wl.heading.as_deref().map(|h| arena_alloc_str(arena_ref, h)),
                    range: wl.range,
                    start_byte: wl.start_byte,
                    end_byte: wl.end_byte,
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
                    start_byte: ml.start_byte,
                    end_byte: ml.end_byte,
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
                    start_byte: xt.start_byte,
                    end_byte: xt.end_byte,
                });
            }
            let xml_tags = xml_tags_builder.into_bump_slice();

            // Arena-allocate frontmatter entries.
            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in frontmatter_owned {
                let key = arena_alloc_str(arena_ref, &fm.key);
                let value = match fm.value {
                    FrontmatterValueOwned::String(s) => {
                        FrontmatterValueEntry::String(arena_alloc_str(arena_ref, &s))
                    }
                    FrontmatterValueOwned::List(items) => {
                        let mut list = BumpVec::new_in(arena_ref);
                        for item in items {
                            list.push(arena_alloc_str(arena_ref, &item));
                        }
                        FrontmatterValueEntry::List(list.into_bump_slice())
                    }
                };
                frontmatter_builder.push(FrontmatterEntry { key, value });
            }
            let frontmatter = frontmatter_builder.into_bump_slice();

            // Arena-allocate aliases (from frontmatter "aliases" key).
            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // Arena-allocate properties entries.
            let mut properties_builder = BumpVec::new_in(arena_ref);
            for prop in properties_owned {
                let key = arena_alloc_str(arena_ref, &prop.key);
                let value = match prop.value {
                    PropertyValueOwned::String(s) => {
                        PropertyValueEntry::String(arena_alloc_str(arena_ref, &s))
                    }
                    PropertyValueOwned::List(items) => {
                        let mut list = BumpVec::new_in(arena_ref);
                        for item in items {
                            list.push(arena_alloc_str(arena_ref, &item));
                        }
                        PropertyValueEntry::List(list.into_bump_slice())
                    }
                    PropertyValueOwned::PageRef(s) => {
                        PropertyValueEntry::PageRef(arena_alloc_str(arena_ref, &s))
                    }
                };
                properties_builder.push(PropertyEntry { key, value });
            }
            let properties = properties_builder.into_bump_slice();

            // Arena-allocate block ref entries.
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for br in block_refs_owned {
                block_refs_builder.push(BlockRefEntry {
                    uuid: arena_alloc_str(arena_ref, &br.uuid),
                    range: br.range,
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

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
                frontmatter,
                aliases,
                properties,
                block_refs,
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
        let line_starts = helpers::byte_offset_line_starts(text);

        // Collect owned data from scan backend before entering self_cell closure
        let ScanAllResult {
            headings: scan_headings,
            links: scan_links,
        } = backend.scan_all(text).unwrap_or_default();
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
