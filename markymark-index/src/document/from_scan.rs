//! [`DocumentIndex::from_scan`] — construct index from a Zig SIMD scan backend.
//!
//! Uses byte-offset based scanning instead of AST parsing. The scan backend
//! provides heading, link, tag, block-id, and code span extraction via SIMD
//! kernels. Frontmatter, properties, and block-refs are not available from
//! the scan path and return empty slices.

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::prelude::*;
use markymark_core::scanner::{ScanBackend, ScanLinkType};
use std::collections::HashMap as StdHashMap;

use super::{
    helpers, BlockEntry, BlockRefEntry, CalloutEntry, CodeSpanEntry, DocumentDependent,
    DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry, FrontmatterEntry, HeadingEntry,
    LinkDefinitionEntry, MarkdownLinkEntry, PropertyEntry, QueryBlockEntry, TagEntry, TaskEntry,
    WikiLinkEntry, XmlTagEntry,
};

impl DocumentIndex {
    /// Build a document index from a scan backend (Zig SIMD path).
    ///
    /// Uses byte-offset based scanning instead of AST parsing. The scan backend
    /// provides heading, link, tag, and block-id extraction via SIMD kernels.
    /// XML tags are not supported by the scan path (returns empty slice).
    pub fn from_scan(text: &str, backend: &dyn ScanBackend) -> Self {
        // Pre-compute line starts for byte-offset → Position conversion
        let line_starts = helpers::byte_offset_line_starts(text);

        // Collect owned data from scan backend before entering self_cell closure.
        // Fall back to independent scans if scan_all fails so that headings
        // and links are never both silently dropped due to one-sided error.
        let (
            scan_headings,
            scan_links,
            scan_code_spans,
            scan_tasks,
            scan_embeds,
            scan_callouts,
            scan_block_refs,
            scan_query_blocks,
            scan_link_definitions,
        ) = match backend.scan_all(text) {
            Ok(result) => (
                result.headings,
                result.links,
                result.code_spans,
                result.tasks,
                result.embeds,
                result.callouts,
                result.block_refs,
                result.query_blocks,
                result.link_definitions,
            ),
            Err(_) => (
                backend.scan_headings(text).unwrap_or_default(),
                backend.scan_links(text).unwrap_or_default(),
                backend.scan_code_spans(text).unwrap_or_default(),
                backend.scan_tasks(text).unwrap_or_default(),
                backend.scan_embeds(text).unwrap_or_default(),
                backend.scan_callouts(text).unwrap_or_default(),
                backend.scan_block_refs(text).unwrap_or_default(),
                backend.scan_query_blocks(text).unwrap_or_default(),
                backend.scan_link_definitions(text).unwrap_or_default(),
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
                let base_slug = super::slugify(&h.text);
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

            // Frontmatter/properties: not available from scan backend
            let frontmatter = BumpVec::<FrontmatterEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let aliases = BumpVec::<&str>::new_in(arena_ref).into_bump_slice();
            let properties = BumpVec::<PropertyEntry<'_>>::new_in(arena_ref).into_bump_slice();

            // --- Tasks ---
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for t in scan_tasks {
                let state = arena_alloc_str(arena_ref, &t.state);
                let text = arena_alloc_str(arena_ref, &t.text);
                let pos = helpers::byte_offset_to_position(&line_starts, t.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, t.end_offset);
                tasks_builder.push(TaskEntry {
                    state,
                    text,
                    range: Range::new(pos, end_pos),
                    start_byte: t.offset as usize,
                    end_byte: t.end_offset as usize,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // --- Embeds ---
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for e in scan_embeds {
                let target = arena_alloc_str(arena_ref, &e.target);
                let pos = helpers::byte_offset_to_position(&line_starts, e.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, e.end_offset);
                embeds_builder.push(EmbedEntry {
                    target,
                    range: Range::new(pos, end_pos),
                    start_byte: e.offset as usize,
                    end_byte: e.end_offset as usize,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // --- Callouts ---
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for c in scan_callouts {
                let callout_type = arena_alloc_str(arena_ref, &c.callout_type);
                let title = c.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let pos = helpers::byte_offset_to_position(&line_starts, c.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, c.end_offset);
                callouts_builder.push(CalloutEntry {
                    callout_type,
                    title,
                    range: Range::new(pos, end_pos),
                    start_byte: c.offset as usize,
                    end_byte: c.end_offset as usize,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // --- Block refs ---
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for br in scan_block_refs {
                let uuid = arena_alloc_str(arena_ref, &br.uuid);
                let pos = helpers::byte_offset_to_position(&line_starts, br.offset);
                // ((uuid)) = 2 + uuid.len() + 2 = uuid.len() + 4
                let end_offset = br.offset + br.uuid.len() as u32 + 4;
                let end_pos = helpers::byte_offset_to_position(&line_starts, end_offset);
                block_refs_builder.push(BlockRefEntry {
                    uuid,
                    range: Range::new(pos, end_pos),
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // --- Query blocks ---
            let mut query_blocks_builder = BumpVec::new_in(arena_ref);
            for qb in scan_query_blocks {
                let query = arena_alloc_str(arena_ref, &qb.query);
                let pos = helpers::byte_offset_to_position(&line_starts, qb.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, qb.end_offset);
                query_blocks_builder.push(QueryBlockEntry {
                    query,
                    range: Range::new(pos, end_pos),
                    start_byte: qb.offset as usize,
                    end_byte: qb.end_offset as usize,
                });
            }
            let query_blocks = query_blocks_builder.into_bump_slice();

            // --- Link definitions ---
            let mut link_defs_builder = BumpVec::new_in(arena_ref);
            for ld in scan_link_definitions {
                let label = arena_alloc_str(arena_ref, &ld.label);
                let url = arena_alloc_str(arena_ref, &ld.url);
                let title = ld.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let pos = helpers::byte_offset_to_position(&line_starts, ld.offset);
                let end_pos = helpers::byte_offset_to_position(&line_starts, ld.end_offset);
                link_defs_builder.push(LinkDefinitionEntry {
                    label,
                    url,
                    title,
                    range: Range::new(pos, end_pos),
                    start_byte: ld.offset as usize,
                    end_byte: ld.end_offset as usize,
                });
            }
            let link_definitions = link_defs_builder.into_bump_slice();

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
