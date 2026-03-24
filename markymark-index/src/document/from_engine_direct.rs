//! [`DocumentIndex::from_engine_result_direct`] — construct index directly from CEngineResult
//! text_blob, bypassing the intermediate EngineExtraction owned Strings.
//!
//! Text fields borrow directly from `DocumentOwner.text_blob` via the self_cell `'a` lifetime.
//! Only frontmatter and aliases (Rust-side data) use arena allocation.

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::{Position, Range};
use markymark_kernels::engine::{read_blob_str, EngineResult};
use markymark_kernels::scan::KernelError;

use super::{
    helpers, BlockKind, BlockRefEntry, CalloutEntry, CodeSpanEntry, ContentBlock,
    DocumentDependent, DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry,
    FrontmatterEntry, FrontmatterOwnedEntry, HeadingEntry, LinkDefinitionEntry, MarkdownLinkEntry,
    PropertyEntry, PropertyValueEntry, QueryBlockEntry, TagEntry, TaskEntry, WikiLinkEntry,
    XmlTagEntry,
};

impl DocumentIndex {
    /// Build a document index by reading `CEngineResult.text_blob` directly — text fields
    /// borrow from `DocumentOwner.text_blob` via the self_cell `'a` lifetime, eliminating
    /// intermediate owned String allocations.
    ///
    /// # Errors
    ///
    /// Returns `KernelError` if the text blob contains invalid UTF-8 or out-of-bounds
    /// offsets.
    pub fn from_engine_result_direct(
        result: &EngineResult,
        fm_owned: Vec<FrontmatterOwnedEntry>,
        aliases_owned: Vec<String>,
    ) -> Result<Self, KernelError> {
        // Pre-closure: collect typed slice data as Copy-type Vecs.
        // These contain only u32/u8 numeric fields — no heap String allocation.
        let headings_raw = result.headings()?.to_vec();
        let links_raw = result.links()?.to_vec();
        let code_spans_raw = result.code_spans()?.to_vec();
        let tags_raw = result.tags()?.to_vec();
        let block_ids_raw = result.block_ids()?.to_vec();
        let tasks_raw = result.tasks()?.to_vec();
        let embeds_raw = result.embeds()?.to_vec();
        let callouts_raw = result.callouts()?.to_vec();
        let block_refs_raw = result.block_refs()?.to_vec();
        let query_blocks_raw = result.query_blocks()?.to_vec();
        let link_defs_raw = result.link_definitions()?.to_vec();
        let props_raw = result.properties()?.to_vec();
        let xml_tags_raw = result.xml_tags()?.to_vec();

        let owner = DocumentOwner {
            arena: DocumentArena::new(),
            source_text: String::new(),
            text_blob: result.text_blob().to_vec(),
        };

        let cell = DocumentIndexCell::try_new(owner, move |owner| {
            let blob = &owner.text_blob;
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            for h in &headings_raw {
                let text = read_blob_str(blob, h.text_offset, h.text_length)?;
                let slug = read_blob_str(blob, h.slug_offset, h.slug_length)?;
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level,
                    range: Range::new(
                        Position::new(h.start_line, h.start_col),
                        Position::new(h.end_line, h.end_col),
                    ),
                });
            }
            let headings = headings_builder.into_bump_slice();

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            // --- Links (wiki + markdown) ---
            let mut wiki_builder = BumpVec::new_in(arena_ref);
            let mut ml_builder = BumpVec::new_in(arena_ref);

            for l in &links_raw {
                let text = read_blob_str(blob, l.text_offset, l.text_length)?;
                let target = read_blob_str(blob, l.target_offset, l.target_length)?;
                let range = Range::new(
                    Position::new(l.start_line, l.start_col),
                    Position::new(l.end_line, l.end_col),
                );

                if l.is_wiki != 0 {
                    // Wiki link: split target on '#' for page/heading
                    let (page, heading) = if let Some(hash_pos) = target.find('#') {
                        (&target[..hash_pos], Some(&target[hash_pos + 1..]))
                    } else {
                        (target, None)
                    };
                    // Alias: display text differs from full target
                    let alias = if text != target { Some(text) } else { None };
                    let start_byte = l.source_offset as usize;
                    let end_byte = if alias.is_some() {
                        start_byte + l.target_length as usize + l.text_length as usize + 5
                    } else {
                        start_byte + l.target_length as usize + 4
                    };
                    wiki_builder.push(WikiLinkEntry {
                        target: page,
                        alias,
                        heading,
                        range,
                        start_byte,
                        end_byte,
                    });
                } else {
                    // Markdown link: split on '#' for url/anchor
                    let (url, anchor) = if let Some(hash_pos) = target.find('#') {
                        (&target[..hash_pos], Some(&target[hash_pos + 1..]))
                    } else {
                        (target, None)
                    };
                    let start_byte = l.source_offset as usize;
                    let end_byte =
                        start_byte + l.text_length as usize + l.target_length as usize + 4;
                    ml_builder.push(MarkdownLinkEntry {
                        text,
                        url,
                        anchor,
                        range,
                        start_byte,
                        end_byte,
                    });
                }
            }
            let wiki_links = wiki_builder.into_bump_slice();
            let markdown_links = ml_builder.into_bump_slice();

            // --- Tags ---
            let mut tags_builder = BumpVec::new_in(arena_ref);
            for t in &tags_raw {
                tags_builder.push(TagEntry {
                    name: read_blob_str(blob, t.name_offset, t.name_length)?,
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs ---
            let mut block_id_map: HashMap<&str, ContentBlock<'_>> = HashMap::new();
            for b in &block_ids_raw {
                let id = read_blob_str(blob, b.id_offset, b.id_length)?;
                let start_byte = b.source_offset as usize;
                let end_byte = start_byte + 1 + b.id_length as usize;
                block_id_map.insert(
                    id,
                    ContentBlock {
                        kind: BlockKind::Paragraph,
                        range: Range::new(
                            Position::new(b.start_line, b.start_col),
                            Position::new(b.end_line, b.end_col),
                        ),
                        start_byte,
                        end_byte,
                        parent_heading: None,
                        block_id: Some(id),
                    },
                );
            }

            // --- Content blocks (empty — matches current hot path behavior) ---
            let content_blocks: &[ContentBlock<'_>] = &[];

            // --- XML Tags ---
            let mut xt_builder = BumpVec::new_in(arena_ref);
            for xt in &xml_tags_raw {
                xt_builder.push(XmlTagEntry {
                    tag_name: read_blob_str(blob, xt.tag_name_offset, xt.tag_name_length)?,
                    attributes: hashbrown::HashMap::new(),
                    is_self_closing: xt.is_self_closing != 0,
                    is_unclosed: xt.is_unclosed != 0,
                    is_inline: xt.is_inline != 0,
                    range: Range::new(
                        Position::new(xt.start_line, xt.start_col),
                        Position::new(xt.end_line, xt.end_col),
                    ),
                    start_byte: xt.source_offset as usize,
                    end_byte: xt.end_offset as usize,
                });
            }
            let xml_tags = xt_builder.into_bump_slice();

            // --- Code spans ---
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for c in &code_spans_raw {
                cs_builder.push(CodeSpanEntry {
                    text: read_blob_str(blob, c.text_offset, c.text_length)?,
                    range: Range::new(
                        Position::new(c.start_line, c.start_col),
                        Position::new(c.end_line, c.end_col),
                    ),
                    start_byte: c.source_offset as usize,
                    end_byte: c.end_offset as usize,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            // --- Frontmatter (Rust-side data — uses arena allocation) ---
            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in fm_owned {
                let key = arena_alloc_str(arena_ref, &fm.key);
                let value = helpers::owned_value_to_arena(fm.value, arena_ref);
                frontmatter_builder.push(FrontmatterEntry { key, value });
            }
            let frontmatter = frontmatter_builder.into_bump_slice();

            // --- Aliases (Rust-side data — uses arena allocation) ---
            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // --- Properties ---
            let mut props_builder = BumpVec::new_in(arena_ref);
            for p in &props_raw {
                let key = read_blob_str(blob, p.key_offset, p.key_length)?;
                let value_str = read_blob_str(blob, p.value_offset, p.value_length)?;
                let value = match p.value_type {
                    1 => {
                        // List: subslices of value_str are &'a str
                        let mut bump_items = BumpVec::new_in(arena_ref);
                        for item in value_str.split(',').map(|s| s.trim()) {
                            bump_items.push(item);
                        }
                        PropertyValueEntry::List(bump_items.into_bump_slice())
                    }
                    2 => {
                        let inner = value_str.trim_start_matches("[[").trim_end_matches("]]");
                        PropertyValueEntry::PageRef(inner)
                    }
                    _ => PropertyValueEntry::String(value_str),
                };
                props_builder.push(PropertyEntry { key, value });
            }
            let properties = props_builder.into_bump_slice();

            // --- Tasks ---
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for t in &tasks_raw {
                // state is &'static str — coerces to &'a str
                let state: &str = if t.state == b'x' || t.state == b'X' {
                    "checked"
                } else {
                    "unchecked"
                };
                tasks_builder.push(TaskEntry {
                    state,
                    text: read_blob_str(blob, t.text_offset, t.text_length)?,
                    range: Range::new(
                        Position::new(t.start_line, t.start_col),
                        Position::new(t.end_line, t.end_col),
                    ),
                    start_byte: t.source_offset as usize,
                    end_byte: t.end_offset as usize,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // --- Embeds ---
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for e in &embeds_raw {
                embeds_builder.push(EmbedEntry {
                    target: read_blob_str(blob, e.target_offset, e.target_length)?,
                    range: Range::new(
                        Position::new(e.start_line, e.start_col),
                        Position::new(e.end_line, e.end_col),
                    ),
                    start_byte: e.source_offset as usize,
                    end_byte: e.end_offset as usize,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // --- Callouts ---
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for c in &callouts_raw {
                let callout_type = read_blob_str(blob, c.type_offset, c.type_length)?;
                let title = if c.title_length == 0 {
                    None
                } else {
                    Some(read_blob_str(blob, c.title_offset, c.title_length)?)
                };
                callouts_builder.push(CalloutEntry {
                    callout_type,
                    title,
                    range: Range::new(
                        Position::new(c.start_line, c.start_col),
                        Position::new(c.end_line, c.end_col),
                    ),
                    start_byte: c.source_offset as usize,
                    end_byte: c.end_offset as usize,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // --- Block refs ---
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for b in &block_refs_raw {
                block_refs_builder.push(BlockRefEntry {
                    uuid: read_blob_str(blob, b.uuid_offset, b.uuid_length)?,
                    range: Range::new(
                        Position::new(b.start_line, b.start_col),
                        Position::new(b.end_line, b.end_col),
                    ),
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // --- Query blocks ---
            let mut qb_builder = BumpVec::new_in(arena_ref);
            for q in &query_blocks_raw {
                qb_builder.push(QueryBlockEntry {
                    query: read_blob_str(blob, q.query_offset, q.query_length)?,
                    range: Range::new(
                        Position::new(q.start_line, q.start_col),
                        Position::new(q.end_line, q.end_col),
                    ),
                    start_byte: q.source_offset as usize,
                    end_byte: q.end_offset as usize,
                });
            }
            let query_blocks = qb_builder.into_bump_slice();

            // --- Link definitions ---
            let mut ld_builder = BumpVec::new_in(arena_ref);
            for l in &link_defs_raw {
                let label = read_blob_str(blob, l.label_offset, l.label_length)?;
                let url = read_blob_str(blob, l.url_offset, l.url_length)?;
                let title = if l.title_length == 0 {
                    None
                } else {
                    Some(read_blob_str(blob, l.title_offset, l.title_length)?)
                };
                ld_builder.push(LinkDefinitionEntry {
                    label,
                    url,
                    title,
                    range: Range::new(
                        Position::new(l.start_line, l.start_col),
                        Position::new(l.end_line, l.end_col),
                    ),
                    start_byte: l.source_offset as usize,
                    end_byte: l.end_offset as usize,
                });
            }
            let link_definitions = ld_builder.into_bump_slice();

            Ok(DocumentDependent {
                headings,
                slug_to_heading,
                content_blocks,
                block_id_map,
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
                embeds,
                tasks,
                callouts,
                query_blocks,
                link_definitions,
            })
        })?;

        Ok(Self { cell })
    }
}
