//! [`DocumentIndex::from_blob`] — construct index from Zig engine binary blob.
//!
//! Reads the flat binary format produced by the Zig [`DocumentEngine`] and
//! constructs a [`DocumentIndex`] with the same content as [`from_scan`] for
//! the same input document.
//!
//! # Blob format (from `zig/src/engine/blob.zig`)
//!
//! ```text
//! [ScanBlobHeader: 64 bytes (v1) | 128 bytes (v2)]
//!   magic(4) version(2) flags(2) content_hash(8)
//!   heading_count(4) link_count(4) tag_count(4) block_id_count(4)
//!   line_count(4) text_pool_size(4) token_estimate(4) total_blob_size(4)
//!   code_span_count(4@48), v2-only counts at 52..84, reserved bytes through 127
//! [BlobHeading × heading_count: 40 bytes each]
//! [BlobLink    × link_count:    40 bytes each]
//! [BlobTag     × tag_count:     24 bytes each]
//! [BlobBlockId × block_id_count: 28 bytes each]
//! [u32         × line_count]    (line_starts — not needed, positions pre-computed)
//! [u8          × text_pool_size] (contiguous text pool)
//! ```
//!
//! [`from_scan`]: super::DocumentIndex::from_scan

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::{Position, Range};

use super::{
    helpers, BlockKind, BlockRefEntry, CalloutEntry, CodeSpanEntry, ContentBlock,
    DocumentDependent, DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry,
    FrontmatterEntry, FrontmatterOwnedEntry, HeadingEntry, LinkDefinitionEntry,
    MarkdownLinkEntry, PropertyEntry, PropertyValueEntry, QueryBlockEntry, TagEntry, TaskEntry,
    WikiLinkEntry, XmlTagEntry,
};

mod decode;
mod header;
mod owned;
use self::decode::decode_owned_data;
pub use self::header::BlobError;
use self::header::*;
use self::owned::DecodedOwnedData;

// ---------------------------------------------------------------------------
// DocumentIndex::from_blob
// ---------------------------------------------------------------------------

impl DocumentIndex {
    /// Build a document index from a Zig engine binary blob.
    ///
    /// Produces a [`DocumentIndex`] equivalent to [`from_scan`] for the same
    /// input text. The blob is the output of `DocumentEngine::get_blob()`.
    ///
    /// XML tags are read directly from the blob (v2 format). V1 blobs
    /// produce zero XML tags.
    ///
    /// # Errors
    ///
    /// Returns [`BlobError`] if the blob is malformed:
    /// - [`BlobError::TooSmall`] — fewer than 64 bytes (v1) or 128 bytes (v2)
    /// - [`BlobError::InvalidMagic`] — magic number mismatch
    /// - [`BlobError::UnsupportedVersion`] — version not in {1, 2}
    /// - [`BlobError::SizeMismatch`] — computed size ≠ actual length
    /// - [`BlobError::TextPoolOutOfBounds`] — an entry's text offset+len overflows pool
    /// - [`BlobError::InvalidUtf8`] — text pool contains invalid UTF-8
    ///
    /// [`from_scan`]: DocumentIndex::from_scan
    pub fn from_blob(data: &[u8]) -> Result<Self, BlobError> {
        Self::from_blob_inner(data, Vec::new(), Vec::new())
    }

    /// Build a document index from a blob with pre-parsed frontmatter.
    ///
    /// Same as [`from_blob`] but accepts owned frontmatter entries and aliases
    /// parsed from the original source text. The blob format does not carry
    /// frontmatter, so this is the only way to populate frontmatter in an
    /// index built from a blob.
    pub fn from_blob_with_frontmatter(
        data: &[u8],
        frontmatter: Vec<FrontmatterOwnedEntry>,
        aliases: Vec<String>,
    ) -> Result<Self, BlobError> {
        Self::from_blob_inner(data, frontmatter, aliases)
    }

    fn from_blob_inner(
        data: &[u8],
        fm_owned: Vec<FrontmatterOwnedEntry>,
        aliases_owned: Vec<String>,
    ) -> Result<Self, BlobError> {
        let header = validate_blob(data)?;
        let offsets = compute_offsets(&header);
        let text_pool =
            &data[offsets.text_pool..offsets.text_pool + header.text_pool_size as usize];

        let DecodedOwnedData {
            headings: headings_owned,
            wiki_links: wiki_owned,
            markdown_links: markdown_owned,
            tags: tags_owned,
            blocks: blocks_owned,
            code_spans: code_spans_owned,
            tasks: tasks_owned,
            embeds: embeds_owned,
            callouts: callouts_owned,
            block_refs: block_refs_owned,
            query_blocks: query_blocks_owned,
            link_definitions: link_defs_owned,
            properties: properties_owned,
            xml_tags: xml_tags_owned,
        } = decode_owned_data(data, &header, &offsets, text_pool)?;

        // ── Build DocumentIndex via self_cell ────────────────────────
        let owner = DocumentOwner {
            arena: DocumentArena::new(),
            source_text: String::new(), // No source text available from blob
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            for h in &headings_owned {
                let text = arena_alloc_str(arena_ref, &h.text);
                // Slug is pre-computed and deduped by Zig engine — use as-is.
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
            for wl in &wiki_owned {
                let target = arena_alloc_str(arena_ref, &wl.target);
                let alias = wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(wl.start_line, wl.start_col);
                let end_pos = Position::new(wl.end_line, wl.end_col);
                let start_byte = wl.source_offset as usize;
                // Compute end_byte matching from_scan's calculation:
                //   [[target]]:        2 + target_len + 2 = target_len + 4
                //   [[target|alias]]:  2 + target_len + 1 + text_len + 2
                //                    = target_len + text_len + 5
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
            for ml in &markdown_owned {
                let text = arena_alloc_str(arena_ref, &ml.text);
                let url = arena_alloc_str(arena_ref, &ml.url);
                let anchor = ml.anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(ml.start_line, ml.start_col);
                let end_pos = Position::new(ml.end_line, ml.end_col);
                let start_byte = ml.source_offset as usize;
                // Compute end_byte matching from_scan:
                //   [text](target): 1 + text_len + 1 + 1 + target_len + 1
                //                 = text_len + target_len + 4
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
            for t in &tags_owned {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &t.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs (Obsidian ^block-id markers) ---
            let mut block_id_map: HashMap<&str, ContentBlock<'_>> = HashMap::new();
            for b in &blocks_owned {
                let id = arena_alloc_str(arena_ref, &b.id);
                let start_pos = Position::new(b.start_line, b.start_col);
                let end_pos = Position::new(b.end_line, b.end_col);
                let start_byte = b.source_offset as usize;
                // end_byte = offset of '^' + 1 (for '^') + id_len
                let end_byte = start_byte + 1 + b.id_len as usize;
                block_id_map.insert(
                    id,
                    ContentBlock {
                        kind: BlockKind::Paragraph,
                        range: Range::new(start_pos, end_pos),
                        start_byte,
                        end_byte,
                        parent_heading: None,
                        block_id: Some(id),
                    },
                );
            }

            // Content blocks: empty (no source text available from blob)
            let content_blocks: &[ContentBlock<'_>] = &[];

            // --- XML Tags (decoded from blob v2) ---
            let mut xt_builder = BumpVec::new_in(arena_ref);
            for xt in &xml_tags_owned {
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
            for cs in &code_spans_owned {
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
                let value = helpers::owned_value_to_arena(fm.value, arena_ref);
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
            for pd in &properties_owned {
                let key = arena_alloc_str(arena_ref, &pd.key);
                let value = match pd.value_type {
                    1 => {
                        // List: split on comma, trim items
                        let items: Vec<&str> = pd.value.split(',').map(|s| s.trim()).collect();
                        let mut bump_items = BumpVec::new_in(arena_ref);
                        for item in items {
                            bump_items.push(arena_alloc_str(arena_ref, item));
                        }
                        PropertyValueEntry::List(bump_items.into_bump_slice())
                    }
                    2 => {
                        // PageRef: strip [[ and ]]
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
            for td in &tasks_owned {
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
            for ed in &embeds_owned {
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
            for cd in &callouts_owned {
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
            for br in &block_refs_owned {
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
            for qb in &query_blocks_owned {
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
            for ld in &link_defs_owned {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
