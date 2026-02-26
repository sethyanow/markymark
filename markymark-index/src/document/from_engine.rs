//! [`DocumentIndex::from_engine_result`] — construct index from CEngineResult conversion.

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::{Position, Range};
use markymark_kernels::engine::EngineExtraction;

use super::{
    helpers, BlockEntry, BlockRefEntry, CalloutEntry, CodeSpanEntry, DocumentDependent,
    DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry, FrontmatterEntry,
    FrontmatterOwnedEntry, FrontmatterValueEntry, FrontmatterValueOwned, HeadingEntry,
    LinkDefinitionEntry, MarkdownLinkEntry, PropertyEntry, PropertyValueEntry, QueryBlockEntry,
    TagEntry, TaskEntry, WikiLinkEntry, XmlTagEntry,
};

impl DocumentIndex {
    /// Build a document index from an owned engine extraction.
    pub fn from_engine_result(data: &EngineExtraction) -> Self {
        Self::from_engine_result_inner(data, Vec::new(), Vec::new())
    }

    /// Build a document index from engine extraction with pre-parsed frontmatter.
    pub fn from_engine_result_with_frontmatter(
        data: &EngineExtraction,
        frontmatter: Vec<FrontmatterOwnedEntry>,
        aliases: Vec<String>,
    ) -> Self {
        Self::from_engine_result_inner(data, frontmatter, aliases)
    }

    fn from_engine_result_inner(
        data: &EngineExtraction,
        fm_owned: Vec<FrontmatterOwnedEntry>,
        aliases_owned: Vec<String>,
    ) -> Self {
        let owner = DocumentOwner {
            arena: DocumentArena::new(),
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            for h in &data.headings {
                let text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &h.slug);
                let start_pos = Position::new(h.start_line, h.start_col);
                let end_pos = Position::new(h.end_line, h.end_col);
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level,
                    range: Range::new(start_pos, end_pos),
                });
            }
            let headings = headings_builder.into_bump_slice();

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            // --- Wiki links ---
            let mut wiki_builder = BumpVec::new_in(arena_ref);
            for wl in &data.wiki_links {
                let target = arena_alloc_str(arena_ref, &wl.target);
                let alias = wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(wl.start_line, wl.start_col);
                let end_pos = Position::new(wl.end_line, wl.end_col);
                let start_byte = wl.source_offset as usize;
                let end_byte = if wl.alias.is_some() {
                    start_byte + wl.target_len as usize + wl.text_len as usize + 5
                } else {
                    start_byte + wl.target_len as usize + 4
                };
                let heading = wl.heading.as_deref().map(|h| arena_alloc_str(arena_ref, h));
                wiki_builder.push(WikiLinkEntry {
                    target,
                    alias,
                    heading,
                    range: Range::new(start_pos, end_pos),
                    start_byte,
                    end_byte,
                });
            }
            let wiki_links = wiki_builder.into_bump_slice();

            // --- Markdown links ---
            let mut ml_builder = BumpVec::new_in(arena_ref);
            for ml in &data.markdown_links {
                let text = arena_alloc_str(arena_ref, &ml.text);
                let url = arena_alloc_str(arena_ref, &ml.url);
                let anchor = ml.anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(ml.start_line, ml.start_col);
                let end_pos = Position::new(ml.end_line, ml.end_col);
                let start_byte = ml.source_offset as usize;
                let end_byte = start_byte + ml.text_len as usize + ml.target_len as usize + 4;
                ml_builder.push(MarkdownLinkEntry {
                    text,
                    url,
                    anchor,
                    range: Range::new(start_pos, end_pos),
                    start_byte,
                    end_byte,
                });
            }
            let markdown_links = ml_builder.into_bump_slice();

            // --- Tags ---
            let mut tags_builder = BumpVec::new_in(arena_ref);
            for t in &data.tags {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &t.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs ---
            let mut blocks: HashMap<&str, BlockEntry<'_>> = HashMap::new();
            for b in &data.block_ids {
                let id = arena_alloc_str(arena_ref, &b.id);
                let start_pos = Position::new(b.start_line, b.start_col);
                let end_pos = Position::new(b.end_line, b.end_col);
                let start_byte = b.source_offset as usize;
                let end_byte = start_byte + 1 + b.id_len as usize;
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: Range::new(start_pos, end_pos),
                        start_byte,
                        end_byte,
                    },
                );
            }

            // --- XML Tags ---
            let mut xt_builder = BumpVec::new_in(arena_ref);
            for xt in &data.xml_tags {
                let tag_name = arena_alloc_str(arena_ref, &xt.tag_name);
                let start_pos = Position::new(xt.start_line, xt.start_col);
                let end_pos = Position::new(xt.end_line, xt.end_col);
                xt_builder.push(XmlTagEntry {
                    tag_name,
                    attributes: hashbrown::HashMap::new(),
                    is_self_closing: xt.is_self_closing,
                    is_unclosed: xt.is_unclosed,
                    is_inline: xt.is_inline,
                    range: Range::new(start_pos, end_pos),
                    start_byte: xt.source_offset as usize,
                    end_byte: xt.end_offset as usize,
                });
            }
            let xml_tags = xt_builder.into_bump_slice();

            // --- Code spans ---
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for cs in &data.code_spans {
                let text = arena_alloc_str(arena_ref, &cs.text);
                let start_pos = Position::new(cs.start_line, cs.start_col);
                let end_pos = Position::new(cs.end_line, cs.end_col);
                cs_builder.push(CodeSpanEntry {
                    text,
                    range: Range::new(start_pos, end_pos),
                    start_byte: cs.source_offset as usize,
                    end_byte: cs.end_offset as usize,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in fm_owned {
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

            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // --- Properties ---
            let mut props_builder = BumpVec::new_in(arena_ref);
            for pd in &data.properties {
                let key = arena_alloc_str(arena_ref, &pd.key);
                let value = match pd.value_type {
                    1 => {
                        let items: Vec<&str> = pd.value.split(',').map(|s| s.trim()).collect();
                        let mut bump_items = BumpVec::new_in(arena_ref);
                        for item in items {
                            bump_items.push(arena_alloc_str(arena_ref, item));
                        }
                        PropertyValueEntry::List(bump_items.into_bump_slice())
                    }
                    2 => {
                        let inner = pd.value.trim_start_matches("[[").trim_end_matches("]]");
                        PropertyValueEntry::PageRef(arena_alloc_str(arena_ref, inner))
                    }
                    _ => PropertyValueEntry::String(arena_alloc_str(arena_ref, &pd.value)),
                };
                props_builder.push(PropertyEntry { key, value });
            }
            let properties = props_builder.into_bump_slice();

            // --- Tasks ---
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for td in &data.tasks {
                let state = arena_alloc_str(arena_ref, &td.state);
                let text = arena_alloc_str(arena_ref, &td.text);
                let start_pos = Position::new(td.start_line, td.start_col);
                let end_pos = Position::new(td.end_line, td.end_col);
                tasks_builder.push(TaskEntry {
                    state,
                    text,
                    range: Range::new(start_pos, end_pos),
                    start_byte: td.source_offset as usize,
                    end_byte: td.end_offset as usize,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // --- Embeds ---
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for ed in &data.embeds {
                let target = arena_alloc_str(arena_ref, &ed.target);
                let start_pos = Position::new(ed.start_line, ed.start_col);
                let end_pos = Position::new(ed.end_line, ed.end_col);
                embeds_builder.push(EmbedEntry {
                    target,
                    range: Range::new(start_pos, end_pos),
                    start_byte: ed.source_offset as usize,
                    end_byte: ed.end_offset as usize,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // --- Callouts ---
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for cd in &data.callouts {
                let callout_type = arena_alloc_str(arena_ref, &cd.callout_type);
                let title = cd.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let start_pos = Position::new(cd.start_line, cd.start_col);
                let end_pos = Position::new(cd.end_line, cd.end_col);
                callouts_builder.push(CalloutEntry {
                    callout_type,
                    title,
                    range: Range::new(start_pos, end_pos),
                    start_byte: cd.source_offset as usize,
                    end_byte: cd.end_offset as usize,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // --- Block refs ---
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for br in &data.block_refs {
                let uuid = arena_alloc_str(arena_ref, &br.uuid);
                let start_pos = Position::new(br.start_line, br.start_col);
                let end_pos = Position::new(br.end_line, br.end_col);
                block_refs_builder.push(BlockRefEntry {
                    uuid,
                    range: Range::new(start_pos, end_pos),
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // --- Query blocks ---
            let mut qb_builder = BumpVec::new_in(arena_ref);
            for qb in &data.query_blocks {
                let query = arena_alloc_str(arena_ref, &qb.query);
                let start_pos = Position::new(qb.start_line, qb.start_col);
                let end_pos = Position::new(qb.end_line, qb.end_col);
                qb_builder.push(QueryBlockEntry {
                    query,
                    range: Range::new(start_pos, end_pos),
                    start_byte: qb.source_offset as usize,
                    end_byte: qb.end_offset as usize,
                });
            }
            let query_blocks = qb_builder.into_bump_slice();

            // --- Link definitions ---
            let mut ld_builder = BumpVec::new_in(arena_ref);
            for ld in &data.link_definitions {
                let label = arena_alloc_str(arena_ref, &ld.label);
                let url = arena_alloc_str(arena_ref, &ld.url);
                let title = ld.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let start_pos = Position::new(ld.start_line, ld.start_col);
                let end_pos = Position::new(ld.end_line, ld.end_col);
                ld_builder.push(LinkDefinitionEntry {
                    label,
                    url,
                    title,
                    range: Range::new(start_pos, end_pos),
                    start_byte: ld.source_offset as usize,
                    end_byte: ld.end_offset as usize,
                });
            }
            let link_definitions = ld_builder.into_bump_slice();

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
                embeds,
                tasks,
                callouts,
                query_blocks,
                link_definitions,
            }
        });

        Self { cell }
    }
}
