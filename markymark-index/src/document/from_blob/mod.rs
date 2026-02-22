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
    helpers, BlockEntry, BlockRefEntry, CalloutEntry, CodeSpanEntry, DocumentDependent,
    DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry, FrontmatterEntry, HeadingEntry,
    LinkDefinitionEntry, MarkdownLinkEntry, PropertyEntry, QueryBlockEntry, TagEntry, TaskEntry,
    WikiLinkEntry, XmlTagEntry, XmlTagOwned,
};

mod decode;
mod header;
mod owned;
use self::decode::decode_owned_data;
use self::header::*;
use self::owned::DecodedOwnedData;
pub use self::header::BlobError;

// ---------------------------------------------------------------------------
// XML tag extraction from raw text (standalone, no tree-sitter needed)
// ---------------------------------------------------------------------------

/// Extract XML/HTML tags from raw markdown text as owned data.
///
/// This uses the single-pass stack-based tokenizer from `markymark_parser`
/// (which does NOT require tree-sitter). Code fences are skipped, and tags
/// with attributes, self-closing tags, and unclosed tags are all handled.
///
/// Used by the engine pipeline (from_blob) to supplement the blob with XML
/// tags that the Zig engine does not extract.
pub fn extract_xml_tags_from_text(source: &str) -> Vec<XmlTagOwned> {
    let arena = bumpalo::Bump::new();
    let tags = markymark_parser::extract_xml_tags(&[], source, &arena);
    tags.into_iter()
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
}

// ---------------------------------------------------------------------------
// DocumentIndex::from_blob
// ---------------------------------------------------------------------------

impl DocumentIndex {
    /// Build a document index from a Zig engine binary blob.
    ///
    /// Produces a [`DocumentIndex`] equivalent to [`from_scan`] for the same
    /// input text. The blob is the output of `DocumentEngine::get_blob()`.
    ///
    /// XML tags are not extracted by the Zig engine; use
    /// [`from_blob_with_xml_tags`] to include them.
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
    /// [`from_blob_with_xml_tags`]: DocumentIndex::from_blob_with_xml_tags
    pub fn from_blob(data: &[u8]) -> Result<Self, BlobError> {
        Self::from_blob_with_xml_tags(data, Vec::new())
    }

    /// Build a document index from a Zig engine binary blob with XML tags.
    ///
    /// Identical to [`from_blob`] but also populates the XML tag entries from
    /// the provided owned data. Use [`extract_xml_tags_from_text`] to obtain
    /// the XML tags from the source text.
    ///
    /// [`from_blob`]: DocumentIndex::from_blob
    pub fn from_blob_with_xml_tags(
        data: &[u8],
        xml_tags_in: Vec<XmlTagOwned>,
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
        } = decode_owned_data(data, &header, &offsets, text_pool)?;

        // ── Build DocumentIndex via self_cell ────────────────────────
        let owner = DocumentOwner {
            arena: DocumentArena::new(),
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

            // --- Block IDs ---
            let mut blocks: HashMap<&str, BlockEntry<'_>> = HashMap::new();
            for b in &blocks_owned {
                let id = arena_alloc_str(arena_ref, &b.id);
                let start_pos = Position::new(b.start_line, b.start_col);
                let end_pos = Position::new(b.end_line, b.end_col);
                let start_byte = b.source_offset as usize;
                // end_byte = offset of '^' + 1 (for '^') + id_len
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

            // --- XML Tags (from supplementary extraction, not in blob) ---
            let mut xt_builder = BumpVec::new_in(arena_ref);
            for xt in &xml_tags_in {
                let tag_name = arena_alloc_str(arena_ref, &xt.tag_name);
                let mut attributes = hashbrown::HashMap::new();
                for (k, v) in &xt.attributes {
                    attributes.insert(arena_alloc_str(arena_ref, k), arena_alloc_str(arena_ref, v));
                }
                xt_builder.push(XmlTagEntry {
                    tag_name,
                    attributes,
                    is_self_closing: xt.is_self_closing,
                    is_unclosed: xt.is_unclosed,
                    range: xt.range,
                    start_byte: xt.start_byte,
                    end_byte: xt.end_byte,
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

            let frontmatter = BumpVec::<FrontmatterEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let aliases = BumpVec::<&str>::new_in(arena_ref).into_bump_slice();
            let properties = BumpVec::<PropertyEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let block_refs = BumpVec::<BlockRefEntry<'_>>::new_in(arena_ref).into_bump_slice();

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

            // Callouts/query_blocks/link_definitions: not yet in blob format
            let callouts = BumpVec::<CalloutEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let query_blocks =
                BumpVec::<QueryBlockEntry<'_>>::new_in(arena_ref).into_bump_slice();
            let link_definitions =
                BumpVec::<LinkDefinitionEntry<'_>>::new_in(arena_ref).into_bump_slice();

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

        Ok(Self { cell })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

