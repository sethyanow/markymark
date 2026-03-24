//! [`DocumentIndex::from_engine_result_direct`] — construct index directly from CEngineResult
//! text_blob, bypassing the intermediate EngineExtraction owned Strings.

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
    /// Build a document index by reading `CEngineResult.text_blob` directly into the
    /// bumpalo arena, bypassing the intermediate `EngineExtraction` owned Strings.
    ///
    /// This eliminates one full copy of all text data compared to the
    /// `to_extraction()` + `from_engine_result_with_frontmatter()` path.
    ///
    /// # Errors
    ///
    /// Returns `KernelError` if the text blob contains invalid UTF-8 or out-of-bounds
    /// offsets — unlike `from_engine_result_inner` which takes pre-validated data.
    pub fn from_engine_result_direct(
        result: &EngineResult,
        fm_owned: Vec<FrontmatterOwnedEntry>,
        aliases_owned: Vec<String>,
    ) -> Result<Self, KernelError> {
        let blob = result.text_blob();

        let owner = DocumentOwner {
            arena: DocumentArena::new(),
            source_text: String::new(),
        };

        // Build all element data from blob reads before entering the cell closure.
        // We collect into Vecs of intermediate tuples, then copy into arena inside the cell.
        //
        // Headings: (text, slug, level, range)
        let mut headings_data = Vec::new();
        for h in result.headings()? {
            let text = read_blob_str(blob, h.text_offset, h.text_length)?;
            let slug = read_blob_str(blob, h.slug_offset, h.slug_length)?;
            headings_data.push((
                text.to_owned(),
                slug.to_owned(),
                h.level,
                Range::new(
                    Position::new(h.start_line, h.start_col),
                    Position::new(h.end_line, h.end_col),
                ),
            ));
        }

        // Links: split into wiki vs markdown, replicating convert_engine_result logic
        let mut wiki_data = Vec::new();
        let mut md_data = Vec::new();
        for l in result.links()? {
            let text = read_blob_str(blob, l.text_offset, l.text_length)?;
            let target = read_blob_str(blob, l.target_offset, l.target_length)?;
            let range = Range::new(
                Position::new(l.start_line, l.start_col),
                Position::new(l.end_line, l.end_col),
            );

            if l.is_wiki != 0 {
                // Wiki link: split target on '#' for page/heading
                let (page, heading) = if let Some(hash_pos) = target.find('#') {
                    (
                        target[..hash_pos].to_owned(),
                        Some(target[hash_pos + 1..].to_owned()),
                    )
                } else {
                    (target.to_owned(), None)
                };
                // Alias: text differs from target
                let alias = if text != target {
                    Some(text.to_owned())
                } else {
                    None
                };
                let start_byte = l.source_offset as usize;
                let end_byte = if alias.is_some() {
                    start_byte + l.target_length as usize + l.text_length as usize + 5
                } else {
                    start_byte + l.target_length as usize + 4
                };
                wiki_data.push((page, alias, heading, range, start_byte, end_byte));
            } else {
                // Markdown link: split on '#' for url/anchor
                let (url, anchor) = if let Some(hash_pos) = target.find('#') {
                    (
                        target[..hash_pos].to_owned(),
                        Some(target[hash_pos + 1..].to_owned()),
                    )
                } else {
                    (target.to_owned(), None)
                };
                let start_byte = l.source_offset as usize;
                let end_byte = start_byte + l.text_length as usize + l.target_length as usize + 4;
                md_data.push((text.to_owned(), url, anchor, range, start_byte, end_byte));
            }
        }

        // Tags
        let mut tags_data = Vec::new();
        for t in result.tags()? {
            tags_data.push(read_blob_str(blob, t.name_offset, t.name_length)?.to_owned());
        }

        // Block IDs
        let mut block_ids_data = Vec::new();
        for b in result.block_ids()? {
            block_ids_data.push((
                read_blob_str(blob, b.id_offset, b.id_length)?.to_owned(),
                b.source_offset,
                b.id_length,
                Range::new(
                    Position::new(b.start_line, b.start_col),
                    Position::new(b.end_line, b.end_col),
                ),
            ));
        }

        // Code spans
        let mut code_spans_data = Vec::new();
        for c in result.code_spans()? {
            code_spans_data.push((
                read_blob_str(blob, c.text_offset, c.text_length)?.to_owned(),
                Range::new(
                    Position::new(c.start_line, c.start_col),
                    Position::new(c.end_line, c.end_col),
                ),
                c.source_offset as usize,
                c.end_offset as usize,
            ));
        }

        // Tasks
        let mut tasks_data = Vec::new();
        for t in result.tasks()? {
            let state = if t.state == b'x' || t.state == b'X' {
                "checked"
            } else {
                "unchecked"
            };
            tasks_data.push((
                state.to_owned(),
                read_blob_str(blob, t.text_offset, t.text_length)?.to_owned(),
                Range::new(
                    Position::new(t.start_line, t.start_col),
                    Position::new(t.end_line, t.end_col),
                ),
                t.source_offset as usize,
                t.end_offset as usize,
            ));
        }

        // Embeds
        let mut embeds_data = Vec::new();
        for e in result.embeds()? {
            embeds_data.push((
                read_blob_str(blob, e.target_offset, e.target_length)?.to_owned(),
                Range::new(
                    Position::new(e.start_line, e.start_col),
                    Position::new(e.end_line, e.end_col),
                ),
                e.source_offset as usize,
                e.end_offset as usize,
            ));
        }

        // Callouts
        let mut callouts_data = Vec::new();
        for c in result.callouts()? {
            let title = if c.title_length == 0 {
                None
            } else {
                Some(read_blob_str(blob, c.title_offset, c.title_length)?.to_owned())
            };
            callouts_data.push((
                read_blob_str(blob, c.type_offset, c.type_length)?.to_owned(),
                title,
                Range::new(
                    Position::new(c.start_line, c.start_col),
                    Position::new(c.end_line, c.end_col),
                ),
                c.source_offset as usize,
                c.end_offset as usize,
            ));
        }

        // Block refs
        let mut block_refs_data = Vec::new();
        for b in result.block_refs()? {
            block_refs_data.push((
                read_blob_str(blob, b.uuid_offset, b.uuid_length)?.to_owned(),
                Range::new(
                    Position::new(b.start_line, b.start_col),
                    Position::new(b.end_line, b.end_col),
                ),
            ));
        }

        // Query blocks
        let mut query_blocks_data = Vec::new();
        for q in result.query_blocks()? {
            query_blocks_data.push((
                read_blob_str(blob, q.query_offset, q.query_length)?.to_owned(),
                Range::new(
                    Position::new(q.start_line, q.start_col),
                    Position::new(q.end_line, q.end_col),
                ),
                q.source_offset as usize,
                q.end_offset as usize,
            ));
        }

        // Link definitions
        let mut link_defs_data = Vec::new();
        for l in result.link_definitions()? {
            let title = if l.title_length == 0 {
                None
            } else {
                Some(read_blob_str(blob, l.title_offset, l.title_length)?.to_owned())
            };
            link_defs_data.push((
                read_blob_str(blob, l.label_offset, l.label_length)?.to_owned(),
                read_blob_str(blob, l.url_offset, l.url_length)?.to_owned(),
                title,
                Range::new(
                    Position::new(l.start_line, l.start_col),
                    Position::new(l.end_line, l.end_col),
                ),
                l.source_offset as usize,
                l.end_offset as usize,
            ));
        }

        // Properties
        let mut props_data = Vec::new();
        for p in result.properties()? {
            props_data.push((
                read_blob_str(blob, p.key_offset, p.key_length)?.to_owned(),
                read_blob_str(blob, p.value_offset, p.value_length)?.to_owned(),
                p.value_type,
            ));
        }

        // XML tags
        let mut xml_tags_data = Vec::new();
        for xt in result.xml_tags()? {
            xml_tags_data.push((
                read_blob_str(blob, xt.tag_name_offset, xt.tag_name_length)?.to_owned(),
                xt.is_self_closing != 0,
                xt.is_unclosed != 0,
                xt.is_inline != 0,
                Range::new(
                    Position::new(xt.start_line, xt.start_col),
                    Position::new(xt.end_line, xt.end_col),
                ),
                xt.source_offset as usize,
                xt.end_offset as usize,
            ));
        }

        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            for (text, slug, level, range) in &headings_data {
                let text = arena_alloc_str(arena_ref, text);
                let slug = arena_alloc_str(arena_ref, slug);
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: *level,
                    range: *range,
                });
            }
            let headings = headings_builder.into_bump_slice();

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            // --- Wiki links ---
            let mut wiki_builder = BumpVec::new_in(arena_ref);
            for (page, alias, heading, range, start_byte, end_byte) in &wiki_data {
                let target = arena_alloc_str(arena_ref, page);
                let alias = alias.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let heading = heading.as_deref().map(|h| arena_alloc_str(arena_ref, h));
                wiki_builder.push(WikiLinkEntry {
                    target,
                    alias,
                    heading,
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let wiki_links = wiki_builder.into_bump_slice();

            // --- Markdown links ---
            let mut ml_builder = BumpVec::new_in(arena_ref);
            for (text, url, anchor, range, start_byte, end_byte) in &md_data {
                let text = arena_alloc_str(arena_ref, text);
                let url = arena_alloc_str(arena_ref, url);
                let anchor = anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                ml_builder.push(MarkdownLinkEntry {
                    text,
                    url,
                    anchor,
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let markdown_links = ml_builder.into_bump_slice();

            // --- Tags ---
            let mut tags_builder = BumpVec::new_in(arena_ref);
            for name in &tags_data {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs ---
            let mut block_id_map: HashMap<&str, ContentBlock<'_>> = HashMap::new();
            for (id, source_offset, id_len, range) in &block_ids_data {
                let id = arena_alloc_str(arena_ref, id);
                let start_byte = *source_offset as usize;
                let end_byte = start_byte + 1 + *id_len as usize;
                block_id_map.insert(
                    id,
                    ContentBlock {
                        kind: BlockKind::Paragraph,
                        range: *range,
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
            for (tag_name, is_self_closing, is_unclosed, is_inline, range, start_byte, end_byte) in
                &xml_tags_data
            {
                xt_builder.push(XmlTagEntry {
                    tag_name: arena_alloc_str(arena_ref, tag_name),
                    attributes: hashbrown::HashMap::new(),
                    is_self_closing: *is_self_closing,
                    is_unclosed: *is_unclosed,
                    is_inline: *is_inline,
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let xml_tags = xt_builder.into_bump_slice();

            // --- Code spans ---
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for (text, range, start_byte, end_byte) in &code_spans_data {
                cs_builder.push(CodeSpanEntry {
                    text: arena_alloc_str(arena_ref, text),
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            // --- Frontmatter ---
            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in fm_owned {
                let key = arena_alloc_str(arena_ref, &fm.key);
                let value = helpers::owned_value_to_arena(fm.value, arena_ref);
                frontmatter_builder.push(FrontmatterEntry { key, value });
            }
            let frontmatter = frontmatter_builder.into_bump_slice();

            // --- Aliases ---
            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // --- Properties ---
            let mut props_builder = BumpVec::new_in(arena_ref);
            for (key, value, value_type) in &props_data {
                let key = arena_alloc_str(arena_ref, key);
                let value = match value_type {
                    1 => {
                        let items: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                        let mut bump_items = BumpVec::new_in(arena_ref);
                        for item in items {
                            bump_items.push(arena_alloc_str(arena_ref, item));
                        }
                        PropertyValueEntry::List(bump_items.into_bump_slice())
                    }
                    2 => {
                        let inner = value.trim_start_matches("[[").trim_end_matches("]]");
                        PropertyValueEntry::PageRef(arena_alloc_str(arena_ref, inner))
                    }
                    _ => PropertyValueEntry::String(arena_alloc_str(arena_ref, value)),
                };
                props_builder.push(PropertyEntry { key, value });
            }
            let properties = props_builder.into_bump_slice();

            // --- Tasks ---
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for (state, text, range, start_byte, end_byte) in &tasks_data {
                tasks_builder.push(TaskEntry {
                    state: arena_alloc_str(arena_ref, state),
                    text: arena_alloc_str(arena_ref, text),
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // --- Embeds ---
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for (target, range, start_byte, end_byte) in &embeds_data {
                embeds_builder.push(EmbedEntry {
                    target: arena_alloc_str(arena_ref, target),
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // --- Callouts ---
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for (callout_type, title, range, start_byte, end_byte) in &callouts_data {
                let callout_type = arena_alloc_str(arena_ref, callout_type);
                let title = title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                callouts_builder.push(CalloutEntry {
                    callout_type,
                    title,
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // --- Block refs ---
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for (uuid, range) in &block_refs_data {
                block_refs_builder.push(BlockRefEntry {
                    uuid: arena_alloc_str(arena_ref, uuid),
                    range: *range,
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // --- Query blocks ---
            let mut qb_builder = BumpVec::new_in(arena_ref);
            for (query, range, start_byte, end_byte) in &query_blocks_data {
                qb_builder.push(QueryBlockEntry {
                    query: arena_alloc_str(arena_ref, query),
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let query_blocks = qb_builder.into_bump_slice();

            // --- Link definitions ---
            let mut ld_builder = BumpVec::new_in(arena_ref);
            for (label, url, title, range, start_byte, end_byte) in &link_defs_data {
                let label = arena_alloc_str(arena_ref, label);
                let url = arena_alloc_str(arena_ref, url);
                let title = title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                ld_builder.push(LinkDefinitionEntry {
                    label,
                    url,
                    title,
                    range: *range,
                    start_byte: *start_byte,
                    end_byte: *end_byte,
                });
            }
            let link_definitions = ld_builder.into_bump_slice();

            DocumentDependent {
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
            }
        });

        Ok(Self { cell })
    }
}
