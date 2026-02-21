//! [`DocumentIndex::from_blob`] — construct index from Zig engine binary blob.
//!
//! Reads the flat binary format produced by the Zig [`DocumentEngine`] and
//! constructs a [`DocumentIndex`] with the same content as [`from_scan`] for
//! the same input document.
//!
//! # Blob format (from `zig/src/engine/blob.zig`)
//!
//! ```text
//! [ScanBlobHeader: 64 bytes]
//!   magic(4) version(2) flags(2) content_hash(8)
//!   heading_count(4) link_count(4) tag_count(4) block_id_count(4)
//!   line_count(4) text_pool_size(4) token_estimate(4) total_blob_size(4)
//!   _reserved(16)
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
    helpers, BlockEntry, BlockRefEntry, CodeSpanEntry, DocumentDependent, DocumentIndex,
    DocumentIndexCell, DocumentOwner, FrontmatterEntry, HeadingEntry, MarkdownLinkEntry,
    PropertyEntry, TagEntry, WikiLinkEntry, XmlTagEntry, XmlTagOwned,
};

// ---------------------------------------------------------------------------
// Constants (must match zig/src/engine/blob.zig)
// ---------------------------------------------------------------------------

const BLOB_MAGIC: u32 = 0x4D4B_5343; // "MKSC"
const BLOB_VERSION: u16 = 1;
const HEADER_SIZE: usize = 64;
const HEADING_SIZE: usize = 40;
const LINK_SIZE: usize = 40;
const TAG_SIZE: usize = 24;
const BLOCK_ID_SIZE: usize = 28;
const CODE_SPAN_SIZE: usize = 32;

// ---------------------------------------------------------------------------
// BlobError
// ---------------------------------------------------------------------------

/// Error type for [`DocumentIndex::from_blob`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobError {
    /// Data is too short to hold a valid blob header (minimum 64 bytes).
    TooSmall,
    /// Magic number does not match `MKSC` (`0x4D4B5343`).
    InvalidMagic,
    /// Blob version field is not supported (expected version 1).
    UnsupportedVersion,
    /// Total blob size field does not match actual data length or computed size.
    SizeMismatch,
    /// A text pool offset + length combination exceeds the text pool bounds.
    TextPoolOutOfBounds,
    /// Text pool bytes are not valid UTF-8.
    InvalidUtf8,
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSmall => write!(f, "blob too small (minimum 64 bytes required for header)"),
            Self::InvalidMagic => {
                write!(f, "invalid blob magic (expected MKSC / 0x4D4B5343)")
            }
            Self::UnsupportedVersion => {
                write!(f, "unsupported blob version (only version 1 is supported)")
            }
            Self::SizeMismatch => write!(
                f,
                "blob size mismatch (header total_blob_size differs from actual or computed size)"
            ),
            Self::TextPoolOutOfBounds => {
                write!(f, "text pool offset + length exceeds text pool bounds")
            }
            Self::InvalidUtf8 => write!(f, "text pool contains invalid UTF-8 bytes"),
        }
    }
}

impl std::error::Error for BlobError {}

// ---------------------------------------------------------------------------
// Low-level byte readers — alignment-safe, little-endian
// ---------------------------------------------------------------------------

#[inline]
fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

#[inline]
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

// ---------------------------------------------------------------------------
// Parsed header
// ---------------------------------------------------------------------------

struct BlobHeader {
    heading_count: u32,
    link_count: u32,
    tag_count: u32,
    block_id_count: u32,
    code_span_count: u32,
    line_count: u32,
    text_pool_size: u32,
}

/// Validate blob magic, version, and size consistency.
///
/// ScanBlobHeader field offsets (extern struct, little-endian):
///   magic(4) version(2) flags(2) content_hash(8)
///   heading_count(4@16) link_count(4@20) tag_count(4@24) block_id_count(4@28)
///   line_count(4@32) text_pool_size(4@36) token_estimate(4@40) total_blob_size(4@44)
///   code_span_count(4@48) _reserved(12@52)
fn validate_blob(data: &[u8]) -> Result<BlobHeader, BlobError> {
    if data.len() < HEADER_SIZE {
        return Err(BlobError::TooSmall);
    }

    let magic = read_u32_le(data, 0);
    if magic != BLOB_MAGIC {
        return Err(BlobError::InvalidMagic);
    }

    let version = read_u16_le(data, 4);
    if version != BLOB_VERSION {
        return Err(BlobError::UnsupportedVersion);
    }

    let heading_count = read_u32_le(data, 16);
    let link_count = read_u32_le(data, 20);
    let tag_count = read_u32_le(data, 24);
    let block_id_count = read_u32_le(data, 28);
    let line_count = read_u32_le(data, 32);
    let text_pool_size = read_u32_le(data, 36);
    let total_blob_size = read_u32_le(data, 44);
    // code_span_count lives at offset 48 — first 4 bytes of what was _reserved.
    // v1 blobs have zeros here, so code_span_count==0 is backward compatible.
    let code_span_count = read_u32_le(data, 48);

    // Compute expected total size via checked arithmetic to prevent overflow.
    let expected = HEADER_SIZE
        .checked_add(
            (heading_count as usize)
                .checked_mul(HEADING_SIZE)
                .ok_or(BlobError::SizeMismatch)?,
        )
        .and_then(|s| s.checked_add((link_count as usize).checked_mul(LINK_SIZE)?))
        .and_then(|s| s.checked_add((tag_count as usize).checked_mul(TAG_SIZE)?))
        .and_then(|s| s.checked_add((block_id_count as usize).checked_mul(BLOCK_ID_SIZE)?))
        .and_then(|s| s.checked_add((code_span_count as usize).checked_mul(CODE_SPAN_SIZE)?))
        .and_then(|s| s.checked_add((line_count as usize).checked_mul(4)?))
        .and_then(|s| s.checked_add(text_pool_size as usize))
        .ok_or(BlobError::SizeMismatch)?;

    if expected != total_blob_size as usize || expected != data.len() {
        return Err(BlobError::SizeMismatch);
    }

    Ok(BlobHeader {
        heading_count,
        link_count,
        tag_count,
        block_id_count,
        code_span_count,
        line_count,
        text_pool_size,
    })
}

// ---------------------------------------------------------------------------
// Section offsets
// ---------------------------------------------------------------------------

struct SectionOffsets {
    headings: usize,
    links: usize,
    tags: usize,
    block_ids: usize,
    code_spans: usize,
    // line_starts skipped — positions are pre-computed in the blob
    text_pool: usize,
}

fn compute_offsets(h: &BlobHeader) -> SectionOffsets {
    let headings = HEADER_SIZE;
    let links = headings + h.heading_count as usize * HEADING_SIZE;
    let tags = links + h.link_count as usize * LINK_SIZE;
    let block_ids = tags + h.tag_count as usize * TAG_SIZE;
    let code_spans = block_ids + h.block_id_count as usize * BLOCK_ID_SIZE;
    let line_starts = code_spans + h.code_span_count as usize * CODE_SPAN_SIZE;
    let text_pool = line_starts + h.line_count as usize * 4;
    SectionOffsets {
        headings,
        links,
        tags,
        block_ids,
        code_spans,
        text_pool,
    }
}

// ---------------------------------------------------------------------------
// Text pool access
// ---------------------------------------------------------------------------

/// Borrow a UTF-8 string from the text pool with bounds and encoding checks.
///
/// Uses checked addition to prevent integer overflow on attacker-controlled
/// `off` + `len` values.
fn pool_str(text_pool: &[u8], off: u32, len: u32) -> Result<&str, BlobError> {
    let start = off as usize;
    let end = start
        .checked_add(len as usize)
        .ok_or(BlobError::TextPoolOutOfBounds)?;
    if end > text_pool.len() {
        return Err(BlobError::TextPoolOutOfBounds);
    }
    std::str::from_utf8(&text_pool[start..end]).map_err(|_| BlobError::InvalidUtf8)
}

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
    /// - [`BlobError::TooSmall`] — fewer than 64 bytes
    /// - [`BlobError::InvalidMagic`] — magic number mismatch
    /// - [`BlobError::UnsupportedVersion`] — version ≠ 1
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

        // Owned intermediate structs — collected before entering self_cell closure.

        struct HeadingData {
            text: String,
            slug: String,
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
            level: u8,
        }

        struct WikiData {
            target: String,
            alias: Option<String>,
            heading: Option<String>,
            source_offset: u32,
            text_len: u32,   // display/alias text length (for end_byte)
            target_len: u32, // page name length (for end_byte)
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
        }

        struct MarkdownData {
            text: String,
            url: String,
            anchor: Option<String>,
            source_offset: u32,
            text_len: u32,   // link text length (for end_byte)
            target_len: u32, // full target length incl. #frag (for end_byte)
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
        }

        struct TagData {
            name: String,
        }

        struct BlockData {
            id: String,
            source_offset: u32,
            id_len: u32,
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
        }

        struct CodeSpanData {
            text: String,
            source_offset: u32,
            end_offset: u32,
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
        }

        let mut headings_owned: Vec<HeadingData> =
            Vec::with_capacity(header.heading_count as usize);
        let mut wiki_owned: Vec<WikiData> = Vec::with_capacity(header.link_count as usize);
        let mut markdown_owned: Vec<MarkdownData> = Vec::with_capacity(header.link_count as usize);
        let mut tags_owned: Vec<TagData> = Vec::with_capacity(header.tag_count as usize);
        let mut blocks_owned: Vec<BlockData> = Vec::with_capacity(header.block_id_count as usize);
        let mut code_spans_owned: Vec<CodeSpanData> =
            Vec::with_capacity(header.code_span_count as usize);

        // ── Headings ────────────────────────────────────────────────
        // BlobHeading layout (40 bytes):
        //   text_off(4@0) text_len(4@4) slug_off(4@8) slug_len(4@12)
        //   source_offset(4@16) start_line(4@20) start_col(4@24)
        //   end_line(4@28) end_col(4@32) level(1@36) _pad(3@37)
        for i in 0..header.heading_count as usize {
            let base = offsets.headings + i * HEADING_SIZE;
            let text_off = read_u32_le(data, base);
            let text_len = read_u32_le(data, base + 4);
            let slug_off = read_u32_le(data, base + 8);
            let slug_len = read_u32_le(data, base + 12);
            let start_line = read_u32_le(data, base + 20);
            let start_col = read_u32_le(data, base + 24);
            let end_line = read_u32_le(data, base + 28);
            let end_col = read_u32_le(data, base + 32);
            let level = read_u8(data, base + 36);

            let text = pool_str(text_pool, text_off, text_len)?.to_owned();
            let slug = pool_str(text_pool, slug_off, slug_len)?.to_owned();

            headings_owned.push(HeadingData {
                text,
                slug,
                start_line,
                start_col,
                end_line,
                end_col,
                level,
            });
        }

        // ── Links ───────────────────────────────────────────────────
        // BlobLink layout (40 bytes):
        //   text_off(4@0) text_len(4@4) target_off(4@8) target_len(4@12)
        //   source_offset(4@16) start_line(4@20) start_col(4@24)
        //   end_line(4@28) end_col(4@32) is_wiki(1@36) _pad(3@37)
        for i in 0..header.link_count as usize {
            let base = offsets.links + i * LINK_SIZE;
            let text_off = read_u32_le(data, base);
            let text_len = read_u32_le(data, base + 4);
            let target_off = read_u32_le(data, base + 8);
            let target_len = read_u32_le(data, base + 12);
            let source_offset = read_u32_le(data, base + 16);
            let start_line = read_u32_le(data, base + 20);
            let start_col = read_u32_le(data, base + 24);
            let end_line = read_u32_le(data, base + 28);
            let end_col = read_u32_le(data, base + 32);
            let is_wiki = read_u8(data, base + 36);

            let text = pool_str(text_pool, text_off, text_len)?;
            let target = pool_str(text_pool, target_off, target_len)?;

            if is_wiki != 0 {
                // Wiki link: text is the display/alias, target may contain
                // a heading anchor (e.g. "page#heading"). Split on '#'.
                let (page, heading) = if let Some(hash_pos) = target.find('#') {
                    (&target[..hash_pos], Some(target[hash_pos + 1..].to_owned()))
                } else {
                    (target, None)
                };
                // Alias is present only when text ≠ full target (before anchor strip).
                // Comparing against `page` (anchor-stripped) was wrong: [[p#h|p]] would
                // see text="p" == page="p" and produce alias=None. marky-d7hh.
                let alias = if text != target {
                    Some(text.to_owned())
                } else {
                    None
                };
                wiki_owned.push(WikiData {
                    alias,
                    heading,
                    target: page.to_owned(),
                    source_offset,
                    text_len,
                    target_len,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                });
            } else {
                // Markdown link: split target on '#' for url + anchor.
                let (url, anchor) = if let Some(hash_pos) = target.find('#') {
                    (
                        target[..hash_pos].to_owned(),
                        Some(target[hash_pos + 1..].to_owned()),
                    )
                } else {
                    (target.to_owned(), None)
                };
                markdown_owned.push(MarkdownData {
                    text: text.to_owned(),
                    url,
                    anchor,
                    source_offset,
                    text_len,
                    target_len,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                });
            }
        }

        // ── Tags ────────────────────────────────────────────────────
        // BlobTag layout (24 bytes):
        //   name_off(4@0) name_len(4@4) source_offset(4@8)
        //   start_line(4@12) start_col(4@16) _pad(4@20)
        for i in 0..header.tag_count as usize {
            let base = offsets.tags + i * TAG_SIZE;
            let name_off = read_u32_le(data, base);
            let name_len = read_u32_le(data, base + 4);
            let name = pool_str(text_pool, name_off, name_len)?.to_owned();
            tags_owned.push(TagData { name });
        }

        // ── Block IDs ───────────────────────────────────────────────
        // BlobBlockId layout (28 bytes):
        //   id_off(4@0) id_len(4@4) source_offset(4@8)
        //   start_line(4@12) start_col(4@16) end_line(4@20) end_col(4@24)
        for i in 0..header.block_id_count as usize {
            let base = offsets.block_ids + i * BLOCK_ID_SIZE;
            let id_off = read_u32_le(data, base);
            let id_len = read_u32_le(data, base + 4);
            let source_offset = read_u32_le(data, base + 8);
            let start_line = read_u32_le(data, base + 12);
            let start_col = read_u32_le(data, base + 16);
            let end_line = read_u32_le(data, base + 20);
            let end_col = read_u32_le(data, base + 24);
            let id = pool_str(text_pool, id_off, id_len)?.to_owned();
            blocks_owned.push(BlockData {
                id,
                source_offset,
                id_len,
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }

        // ── Code Spans ────────────────────────────────────────────────
        // BlobCodeSpan layout (32 bytes):
        //   text_off(4@0) text_len(4@4) source_offset(4@8) end_offset(4@12)
        //   start_line(4@16) start_col(4@20) end_line(4@24) end_col(4@28)
        for i in 0..header.code_span_count as usize {
            let base = offsets.code_spans + i * CODE_SPAN_SIZE;
            let text_off = read_u32_le(data, base);
            let text_len = read_u32_le(data, base + 4);
            let source_offset = read_u32_le(data, base + 8);
            let end_offset = read_u32_le(data, base + 12);
            let start_line = read_u32_le(data, base + 16);
            let start_col = read_u32_le(data, base + 20);
            let end_line = read_u32_le(data, base + 24);
            let end_col = read_u32_le(data, base + 28);
            let text = pool_str(text_pool, text_off, text_len)?.to_owned();
            code_spans_owned.push(CodeSpanData {
                text,
                source_offset,
                end_offset,
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }

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

        Ok(Self { cell })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::DocumentIndex;
    use super::*;
    use markymark_kernels::engine::DocumentEngine;

    /// Helper: create a blob from markdown text via the real Zig engine.
    fn blob_for(text: &str) -> Vec<u8> {
        let engine = DocumentEngine::new(text).expect("engine creation failed");
        engine.get_blob().expect("get_blob failed").data().to_vec()
    }

    // ── Engine-backed tests (tests 1–9, 14–15) ───────────────────────────

    #[test]
    fn test_from_blob_empty_document() {
        let blob = blob_for("");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert!(index.headings().is_empty());
        assert!(index.wiki_links().is_empty());
        assert!(index.tags().is_empty());
        assert!(index.markdown_links().is_empty());
        assert!(index.toc().is_empty());
        assert_eq!(index.block_ids().count(), 0);
    }

    #[test]
    fn test_from_blob_single_heading() {
        let blob = blob_for("# Hello\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.headings().len(), 1);
        assert_eq!(index.headings()[0].text, "Hello");
        assert_eq!(index.headings()[0].slug, "hello");
        assert_eq!(index.headings()[0].level, 1);
        // Heading should be reachable by slug
        assert!(index.heading_by_slug("hello").is_some());
    }

    #[test]
    fn test_from_blob_multiple_headings_with_dedup_slugs() {
        let blob = blob_for("# Title\n\n# Title\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.headings().len(), 2);
        assert_eq!(index.headings()[0].slug, "title");
        assert_eq!(index.headings()[1].slug, "title-1");
    }

    #[test]
    fn test_from_blob_wiki_link() {
        let blob = blob_for("[[My Page]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        assert_eq!(index.wiki_links()[0].target, "My Page");
        assert_eq!(index.wiki_links()[0].alias, None);
        assert_eq!(index.wiki_links()[0].heading, None);
    }

    #[test]
    fn test_from_blob_wiki_link_with_alias() {
        let blob = blob_for("[[target|display]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        assert_eq!(index.wiki_links()[0].target, "target");
        assert_eq!(index.wiki_links()[0].alias, Some("display"));
    }

    /// Generic wiki link with heading — verify both target and heading are extracted.
    #[test]
    fn test_from_blob_wiki_link_with_heading() {
        let blob = blob_for("[[page#heading]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "page");
        assert_eq!(wl.heading, Some("heading"));
    }

    /// marky-d7hh: [[page#heading|page]] — alias text matches the page part.
    /// from_blob was comparing text != page (anchor-stripped), so "page" == "page"
    /// incorrectly produced alias=None. Fix: compare text != target (full target).
    #[test]
    fn test_from_blob_wiki_link_with_heading_and_matching_alias() {
        let blob = blob_for("[[page#heading|page]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        let wl = &index.wiki_links()[0];
        assert_eq!(
            wl.target, "page",
            "target should be page-only (anchor stripped)"
        );
        assert_eq!(
            wl.heading,
            Some("heading"),
            "heading field should be populated"
        );
        assert_eq!(
            wl.alias,
            Some("page"),
            "alias should be Some when text differs from full target"
        );
    }

    /// marky-d7hh: [[page#heading|other]] — alias text differs from both page and full target.
    /// This case was already correct before the fix; regression guard.
    #[test]
    fn test_from_blob_wiki_link_with_heading_and_different_alias() {
        let blob = blob_for("[[page#heading|other]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "page");
        assert_eq!(wl.heading, Some("heading"));
        assert_eq!(wl.alias, Some("other"));
    }

    /// marky-d7hh: [[page#heading]] — no alias, anchor only.
    /// from_blob was comparing text="page#heading" != page="page" → alias=Some("page#heading").
    /// Fix: text="page#heading" != target="page#heading" → False → alias=None.
    #[test]
    fn test_from_blob_wiki_link_with_heading_no_alias() {
        let blob = blob_for("[[page#heading]]\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.wiki_links().len(), 1);
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "page");
        assert_eq!(wl.heading, Some("heading"));
        assert_eq!(wl.alias, None, "no pipe separator means no alias");
    }

    #[test]
    fn test_from_blob_markdown_link_with_anchor() {
        let blob = blob_for("[text](url.md#frag)\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.markdown_links().len(), 1);
        assert_eq!(index.markdown_links()[0].text, "text");
        assert_eq!(index.markdown_links()[0].url, "url.md");
        assert_eq!(index.markdown_links()[0].anchor, Some("frag"));
    }

    #[test]
    fn test_from_blob_tags() {
        let blob = blob_for("text #alpha #beta\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert!(index.tags().len() >= 2);
        assert!(index.tags().iter().any(|t| t.name == "alpha"));
        assert!(index.tags().iter().any(|t| t.name == "beta"));
    }

    #[test]
    fn test_from_blob_block_ids() {
        let blob = blob_for("content ^my-id\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert!(
            index.block_by_id("my-id").is_some(),
            "block ID 'my-id' should be indexed"
        );
    }

    #[test]
    fn test_from_blob_toc_and_outline() {
        let blob = blob_for("# A\n\n## B\n\n### C\n");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

        let toc = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 2);

        let outline = index.outline();
        assert_eq!(outline.children.len(), 1, "root has 1 L1 child");
        assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "A");
        assert_eq!(outline.children[0].children.len(), 1, "A has 1 L2 child");
    }

    // ── Validation rejection tests (tests 10–13) ─────────────────────────

    #[test]
    fn test_from_blob_rejects_invalid_magic() {
        let mut buf = [0u8; 64];
        // Write wrong magic in little-endian
        buf[0] = 0xEF;
        buf[1] = 0xBE;
        buf[2] = 0xAD;
        buf[3] = 0xDE;
        // Write valid BLOB_VERSION (1)
        buf[4] = 1;
        buf[5] = 0;
        // total_blob_size = 64
        buf[44] = 64;
        assert!(matches!(
            DocumentIndex::from_blob(&buf),
            Err(BlobError::InvalidMagic)
        ));
    }

    #[test]
    fn test_from_blob_rejects_bad_version() {
        let mut buf = [0u8; 64];
        // Write correct magic: 0x4D4B5343 in little-endian
        buf[0] = 0x43;
        buf[1] = 0x53;
        buf[2] = 0x4B;
        buf[3] = 0x4D;
        // Write version = 99
        buf[4] = 99;
        buf[5] = 0;
        assert!(matches!(
            DocumentIndex::from_blob(&buf),
            Err(BlobError::UnsupportedVersion)
        ));
    }

    #[test]
    fn test_from_blob_rejects_truncated() {
        let buf = [0u8; 32];
        assert!(matches!(
            DocumentIndex::from_blob(&buf),
            Err(BlobError::TooSmall)
        ));
    }

    #[test]
    fn test_from_blob_rejects_size_mismatch() {
        // Build a valid minimal blob (header only) but corrupt total_blob_size.
        let blob = blob_for("");
        assert_eq!(blob.len(), 64);
        let mut corrupt = blob.clone();
        // Set total_blob_size to 128 (doesn't match actual 64 bytes)
        corrupt[44] = 128;
        corrupt[45] = 0;
        corrupt[46] = 0;
        corrupt[47] = 0;
        assert!(matches!(
            DocumentIndex::from_blob(&corrupt),
            Err(BlobError::SizeMismatch)
        ));
    }

    // ── Parity test (test 14) ─────────────────────────────────────────────

    #[test]
    fn test_from_blob_parity_with_from_scan() {
        // Compare blob (DocumentEngine/md4c) vs from_scan with Md4cScanBackend.
        // Both use md4c extraction so offsets are identical.
        // ZigScanBackend uses SIMD scanner with different offset conventions.
        use markymark_core::scanner::Md4cScanBackend;

        let text =
            "# Main Heading\n\n## Sub Heading\n\n[[Wiki Link]]\n[[Page|Alias]]\n[md](url.md#sec)\n#tag1 #tag2\ncontent ^block1\n";

        // Build via engine blob path
        let blob = blob_for(text);
        let blob_idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");

        // Build via md4c scan path (same extraction as DocumentEngine)
        let backend = Md4cScanBackend;
        let scan_idx = DocumentIndex::from_scan(text, &backend);

        // Headings: text, slug, level, range must match exactly
        let blob_headings = blob_idx.headings();
        let scan_headings = scan_idx.headings();
        assert_eq!(blob_headings.len(), scan_headings.len(), "heading count");
        for (b, s) in blob_headings.iter().zip(scan_headings.iter()) {
            assert_eq!(b.text, s.text, "heading text");
            assert_eq!(b.slug, s.slug, "heading slug");
            assert_eq!(b.level, s.level, "heading level");
            assert_eq!(b.range, s.range, "heading range for '{}'", b.text);
        }

        // Wiki links: target, alias, range must match
        let blob_wl = blob_idx.wiki_links();
        let scan_wl = scan_idx.wiki_links();
        assert_eq!(blob_wl.len(), scan_wl.len(), "wiki link count");
        for (b, s) in blob_wl.iter().zip(scan_wl.iter()) {
            assert_eq!(b.target, s.target, "wiki link target");
            assert_eq!(b.alias, s.alias, "wiki link alias");
            assert_eq!(b.range, s.range, "wiki link range for '{}'", b.target);
        }

        // Markdown links: text, url, anchor, range must match
        let blob_ml = blob_idx.markdown_links();
        let scan_ml = scan_idx.markdown_links();
        assert_eq!(blob_ml.len(), scan_ml.len(), "markdown link count");
        for (b, s) in blob_ml.iter().zip(scan_ml.iter()) {
            assert_eq!(b.text, s.text, "markdown link text");
            assert_eq!(b.url, s.url, "markdown link url");
            assert_eq!(b.anchor, s.anchor, "markdown link anchor");
            assert_eq!(b.range, s.range, "markdown link range for '{}'", b.text);
        }

        // Tags: names must match (order may differ — use set comparison)
        let blob_tags: std::collections::HashSet<&str> =
            blob_idx.tags().iter().map(|t| t.name).collect();
        let scan_tags: std::collections::HashSet<&str> =
            scan_idx.tags().iter().map(|t| t.name).collect();
        assert_eq!(blob_tags, scan_tags, "tag names");

        // Block IDs: must match
        let blob_blocks: std::collections::HashSet<&str> = blob_idx.block_ids().collect();
        let scan_blocks: std::collections::HashSet<&str> = scan_idx.block_ids().collect();
        assert_eq!(blob_blocks, scan_blocks, "block IDs");
    }

    // ── Mixed document test (test 15) ─────────────────────────────────────

    #[test]
    fn test_from_blob_mixed_document() {
        let text = concat!(
            "# Title One\n\n",
            "## Section A\n\n",
            "## Section A\n\n", // duplicate slug → dedup
            "[[Simple Link]]\n",
            "[[Page Name|Display Text]]\n",
            "[Click here](https://example.com)\n",
            "[Anchored](doc.md#section)\n",
            "tags: #alpha #beta #gamma\n",
            "block one ^id-one\n",
            "block two ^id-two\n",
        );
        let blob = blob_for(text);
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

        // Headings with deduplication
        assert_eq!(index.headings().len(), 3);
        assert_eq!(index.headings()[0].slug, "title-one");
        assert_eq!(index.headings()[1].slug, "section-a");
        assert_eq!(index.headings()[2].slug, "section-a-1");

        // Wiki links
        assert_eq!(index.wiki_links().len(), 2);
        assert!(index
            .wiki_links()
            .iter()
            .any(|w| w.target == "Simple Link" && w.alias.is_none()));
        assert!(index
            .wiki_links()
            .iter()
            .any(|w| w.target == "Page Name" && w.alias == Some("Display Text")));

        // Markdown links
        assert_eq!(index.markdown_links().len(), 2);
        assert!(index
            .markdown_links()
            .iter()
            .any(|m| m.url == "https://example.com" && m.anchor.is_none()));
        assert!(index
            .markdown_links()
            .iter()
            .any(|m| m.url == "doc.md" && m.anchor == Some("section")));

        // Tags
        assert!(index.tags().iter().any(|t| t.name == "alpha"));
        assert!(index.tags().iter().any(|t| t.name == "beta"));
        assert!(index.tags().iter().any(|t| t.name == "gamma"));

        // Block IDs
        assert!(index.block_by_id("id-one").is_some());
        assert!(index.block_by_id("id-two").is_some());

        // TOC
        let toc = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 1);
    }

    // ── XML tag supplementary tests (test 16–17) ─────────────────────────

    #[test]
    fn test_from_blob_with_xml_tags() {
        let text = "# Heading\n\n<agent>content</agent>\n\n<goal>win</goal>\n";
        let blob = blob_for(text);
        let xml_tags = super::extract_xml_tags_from_text(text);

        assert!(xml_tags.len() >= 2, "should extract agent and goal tags");

        let index =
            DocumentIndex::from_blob_with_xml_tags(&blob, xml_tags).expect("from_blob failed");

        // Headings still work
        assert_eq!(index.headings().len(), 1);
        assert_eq!(index.headings()[0].text, "Heading");

        // XML tags are populated
        assert!(
            !index.xml_tags().is_empty(),
            "xml_tags should not be empty when provided"
        );
        let tag_names: Vec<&str> = index.xml_tags().iter().map(|xt| xt.tag_name).collect();
        assert!(
            tag_names.contains(&"agent"),
            "should include 'agent'; got: {:?}",
            tag_names
        );
        assert!(
            tag_names.contains(&"goal"),
            "should include 'goal'; got: {:?}",
            tag_names
        );
    }

    #[test]
    fn test_extract_xml_tags_from_text_basic() {
        let text = "<agent>hello</agent>\n<goal>win</goal>\n<routing>path</routing>\n";
        let tags = super::extract_xml_tags_from_text(text);
        let names: Vec<&str> = tags.iter().map(|t| t.tag_name.as_str()).collect();
        assert!(
            names.contains(&"agent"),
            "should find agent; got: {:?}",
            names
        );
        assert!(
            names.contains(&"goal"),
            "should find goal; got: {:?}",
            names
        );
        assert!(
            names.contains(&"routing"),
            "should find routing; got: {:?}",
            names
        );
    }

    // ── Golden blob roundtrip test ────────────────────────────────────────

    /// Canonical markdown input used for the golden blob.
    ///
    /// Covers all element types: headings (with slug dedup), wiki links
    /// (plain and aliased), markdown links (plain and anchored), tags, and
    /// block IDs.
    ///
    /// Generated blob is committed at testdata/golden_v1.blob.
    /// Blob version: 1  |  Magic: 0x4D4B5343 ("MKSC")
    ///
    /// To regenerate: `cargo test -p markymark-index generate_golden_blob -- --include-ignored`
    const GOLDEN_MARKDOWN: &str = concat!(
        "# Title One\n\n",
        "## Section A\n\n",
        "## Section A\n\n",
        "[[Simple Link]]\n",
        "[[Page Name|Display Text]]\n",
        "[Click here](https://example.com)\n",
        "[Anchored](doc.md#section)\n",
        "tags: #alpha #beta #gamma\n",
        "block one ^id-one\n",
        "block two ^id-two\n",
    );

    /// One-off generator — run with:
    ///   cargo test -p markymark-index generate_golden_blob -- --include-ignored
    #[test]
    #[ignore]
    fn generate_golden_blob() {
        let blob = blob_for(GOLDEN_MARKDOWN);
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let path = std::path::Path::new(&manifest_dir).join("src/document/testdata/golden_v1.blob");
        std::fs::write(&path, &blob).expect("failed to write golden blob");
        println!("Wrote {} bytes to {}", blob.len(), path.display());
    }

    #[test]
    fn test_golden_blob_roundtrip() {
        let blob = include_bytes!("testdata/golden_v1.blob");

        // validate_blob() must succeed and header counts must match the input
        let header = validate_blob(blob).expect("validate_blob failed on golden blob");
        assert_eq!(header.heading_count, 3, "expected 3 headings");
        assert_eq!(
            header.link_count, 4,
            "expected 4 links (2 wiki + 2 markdown)"
        );
        assert_eq!(header.tag_count, 3, "expected 3 tags");
        assert_eq!(header.block_id_count, 2, "expected 2 block IDs");

        // from_blob() must succeed
        let index = DocumentIndex::from_blob(blob).expect("from_blob failed on golden blob");

        // Headings: dedup slug check
        assert_eq!(index.headings().len(), 3);
        assert_eq!(index.headings()[0].text, "Title One");
        assert_eq!(index.headings()[0].slug, "title-one");
        assert_eq!(index.headings()[0].level, 1);
        assert_eq!(index.headings()[1].text, "Section A");
        assert_eq!(index.headings()[1].slug, "section-a");
        assert_eq!(index.headings()[1].level, 2);
        assert_eq!(index.headings()[2].slug, "section-a-1");

        // Wiki links
        assert_eq!(index.wiki_links().len(), 2);
        assert!(
            index
                .wiki_links()
                .iter()
                .any(|w| w.target == "Simple Link" && w.alias.is_none()),
            "expected wiki link to 'Simple Link'"
        );
        assert!(
            index
                .wiki_links()
                .iter()
                .any(|w| w.target == "Page Name" && w.alias == Some("Display Text")),
            "expected aliased wiki link to 'Page Name'"
        );

        // Markdown links
        assert_eq!(index.markdown_links().len(), 2);
        assert!(
            index
                .markdown_links()
                .iter()
                .any(|m| m.url == "https://example.com" && m.anchor.is_none()),
            "expected markdown link to https://example.com"
        );
        assert!(
            index
                .markdown_links()
                .iter()
                .any(|m| m.url == "doc.md" && m.anchor == Some("section")),
            "expected anchored markdown link to doc.md#section"
        );

        // Tags
        assert!(
            index.tags().iter().any(|t| t.name == "alpha"),
            "expected tag 'alpha'"
        );
        assert!(
            index.tags().iter().any(|t| t.name == "beta"),
            "expected tag 'beta'"
        );
        assert!(
            index.tags().iter().any(|t| t.name == "gamma"),
            "expected tag 'gamma'"
        );

        // Block IDs
        assert!(
            index.block_by_id("id-one").is_some(),
            "expected block id 'id-one'"
        );
        assert!(
            index.block_by_id("id-two").is_some(),
            "expected block id 'id-two'"
        );
    }

    #[test]
    fn test_blob_error_display_messages() {
        // Each variant must produce a non-empty, distinct human-readable message.
        let cases: &[(BlobError, &str)] = &[
            (BlobError::TooSmall, "64 bytes"),
            (BlobError::InvalidMagic, "MKSC"),
            (BlobError::UnsupportedVersion, "version 1"),
            (BlobError::SizeMismatch, "size mismatch"),
            (BlobError::TextPoolOutOfBounds, "text pool"),
            (BlobError::InvalidUtf8, "UTF-8"),
        ];
        for (err, expected_substr) in cases {
            let msg = format!("{}", err);
            assert!(
                msg.contains(expected_substr),
                "Display for {err:?} = {msg:?}; expected to contain {expected_substr:?}"
            );
        }
    }

    #[test]
    fn test_blob_error_is_std_error() {
        // BlobError must be usable as Box<dyn std::error::Error>.
        fn accepts_error(_: &dyn std::error::Error) {}
        accepts_error(&BlobError::InvalidMagic);

        // Must be usable with ? in Box<dyn Error> context.
        fn returns_box_err() -> Result<(), Box<dyn std::error::Error>> {
            let data: &[u8] = &[0u8; 4]; // too small
            DocumentIndex::from_blob(data)?; // should propagate BlobError::TooSmall
            Ok(())
        }
        assert!(returns_box_err().is_err());
    }

    #[test]
    fn test_blob_error_display_all_variants_distinct() {
        // All 6 variant messages must be distinct (catch copy-paste errors).
        use std::collections::HashSet;
        let messages: HashSet<String> = [
            BlobError::TooSmall,
            BlobError::InvalidMagic,
            BlobError::UnsupportedVersion,
            BlobError::SizeMismatch,
            BlobError::TextPoolOutOfBounds,
            BlobError::InvalidUtf8,
        ]
        .iter()
        .map(|e| format!("{e}"))
        .collect();
        assert_eq!(
            messages.len(),
            6,
            "All BlobError variants must have distinct Display messages"
        );
    }

    // ── Code span tests (marky-vsh2) ────────────────────────────────────

    #[test]
    fn test_from_blob_code_spans_basic() {
        let blob = blob_for("Hello `world` end");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        let cs = index.code_spans();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].text, "world");
        assert_eq!(cs[0].start_byte, 6); // offset of opening backtick
        assert_eq!(cs[0].end_byte, 13); // past closing backtick
    }

    #[test]
    fn test_from_blob_code_spans_multiple() {
        let blob = blob_for("`a` and `b`");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        let cs = index.code_spans();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].text, "a");
        assert_eq!(cs[1].text, "b");
        assert!(cs[1].start_byte > cs[0].start_byte);
    }

    #[test]
    fn test_from_blob_code_spans_empty() {
        let blob = blob_for("No code here");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert!(index.code_spans().is_empty());
    }

    #[test]
    fn test_from_blob_code_spans_in_heading() {
        let blob = blob_for("# Title `code` end");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert_eq!(index.headings().len(), 1);
        let cs = index.code_spans();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].text, "code");
    }

    #[test]
    fn test_from_blob_code_spans_backward_compat() {
        // A v1 blob (code_span_count=0) should still parse with empty code_spans.
        // The blob_for helper uses the current engine which now sets code_span_count.
        // To test backward compat, use a document with no code spans.
        let blob = blob_for("# Just a heading\n\nSome text.");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        assert!(index.code_spans().is_empty());
        assert_eq!(index.headings().len(), 1);
    }

    #[test]
    fn test_from_blob_code_span_positions() {
        let blob = blob_for("line1\n`code`\nline3");
        let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
        let cs = index.code_spans();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].text, "code");
        // Code span is on line 1 (0-indexed), col 0
        assert_eq!(cs[0].range.start.line, 1);
        assert_eq!(cs[0].range.start.character, 0);
    }

    #[test]
    fn test_from_blob_code_span_parity_with_from_scan() {
        use markymark_core::scanner::Md4cScanBackend;

        let text = "# Heading\n\nSome `code` and `more code` here.\n\n[link](url)";
        let blob = blob_for(text);
        let blob_index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

        let backend = Md4cScanBackend;
        let scan_index = DocumentIndex::from_scan(text, &backend);

        let blob_cs = blob_index.code_spans();
        let scan_cs = scan_index.code_spans();
        assert_eq!(blob_cs.len(), scan_cs.len(), "code span count mismatch");
        for (b, s) in blob_cs.iter().zip(scan_cs.iter()) {
            assert_eq!(b.text, s.text, "code span text mismatch");
            assert_eq!(b.start_byte, s.start_byte, "start_byte mismatch");
            assert_eq!(b.end_byte, s.end_byte, "end_byte mismatch");
            assert_eq!(
                b.range.start.line, s.range.start.line,
                "start line mismatch"
            );
            assert_eq!(
                b.range.start.character, s.range.start.character,
                "start col mismatch"
            );
        }
    }
}
