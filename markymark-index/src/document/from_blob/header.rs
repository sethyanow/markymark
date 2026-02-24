// ---------------------------------------------------------------------------
// Constants (must match zig/src/engine/blob.zig)
// ---------------------------------------------------------------------------

pub(super) const BLOB_MAGIC: u32 = 0x4D4B_5343; // "MKSC"
pub(super) const BLOB_VERSION_V1: u16 = 1;
pub(super) const BLOB_VERSION_V2: u16 = 2;
pub(super) const V1_HEADER_SIZE: usize = 64;
pub(super) const V2_HEADER_SIZE: usize = 128;
pub(super) const HEADING_SIZE: usize = 40;
pub(super) const LINK_SIZE: usize = 40;
pub(super) const TAG_SIZE: usize = 24;
pub(super) const BLOCK_ID_SIZE: usize = 28;
pub(super) const CODE_SPAN_SIZE: usize = 32;
pub(super) const TASK_SIZE: usize = 36;
pub(super) const EMBED_SIZE: usize = 32;
pub(super) const CALLOUT_SIZE: usize = 40;
pub(super) const BLOCK_REF_SIZE: usize = 28;
pub(super) const QUERY_BLOCK_SIZE: usize = 32;
pub(super) const LINK_DEF_SIZE: usize = 48;
pub(super) const PROPERTY_SIZE: usize = 20;
pub(super) const XML_TAG_SIZE: usize = 40;

// ---------------------------------------------------------------------------
// BlobError
// ---------------------------------------------------------------------------

/// Error type for [`DocumentIndex::from_blob`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobError {
    /// Data is too short to hold a valid blob header (minimum 64 bytes for v1, 128 bytes for v2).
    TooSmall,
    /// Magic number does not match `MKSC` (`0x4D4B5343`).
    InvalidMagic,
    /// Blob version field is not supported (expected versions 1 or 2).
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
            Self::TooSmall => write!(
                f,
                "blob too small (minimum 64 bytes for v1 or 128 bytes for v2 header)"
            ),
            Self::InvalidMagic => {
                write!(f, "invalid blob magic (expected MKSC / 0x4D4B5343)")
            }
            Self::UnsupportedVersion => {
                write!(
                    f,
                    "unsupported blob version (only versions 1 and 2 are supported)"
                )
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
pub(super) fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

#[inline]
pub(super) fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub(super) fn read_u32_le(data: &[u8], offset: usize) -> u32 {
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

pub(super) struct BlobHeader {
    pub(super) header_size: usize,
    pub(super) heading_count: u32,
    pub(super) link_count: u32,
    pub(super) tag_count: u32,
    pub(super) block_id_count: u32,
    pub(super) code_span_count: u32,
    pub(super) task_count: u32,
    pub(super) embed_count: u32,
    pub(super) callout_count: u32,
    pub(super) block_ref_count: u32,
    pub(super) query_block_count: u32,
    pub(super) link_def_count: u32,
    pub(super) property_count: u32,
    pub(super) xml_tag_count: u32,
    pub(super) line_count: u32,
    pub(super) text_pool_size: u32,
}

/// Validate blob magic, version, and size consistency.
///
/// ScanBlobHeader field offsets (extern struct, little-endian):
///   magic(4) version(2) flags(2) content_hash(8)
///   heading_count(4@16) link_count(4@20) tag_count(4@24) block_id_count(4@28)
///   line_count(4@32) text_pool_size(4@36) token_estimate(4@40) total_blob_size(4@44)
///   code_span_count(4@48) + v2 count fields at 52..84, then reserved
pub(super) fn validate_blob(data: &[u8]) -> Result<BlobHeader, BlobError> {
    if data.len() < V1_HEADER_SIZE {
        return Err(BlobError::TooSmall);
    }

    let magic = read_u32_le(data, 0);
    if magic != BLOB_MAGIC {
        return Err(BlobError::InvalidMagic);
    }

    let version = read_u16_le(data, 4);
    let header_size = match version {
        BLOB_VERSION_V1 => V1_HEADER_SIZE,
        BLOB_VERSION_V2 => {
            if data.len() < V2_HEADER_SIZE {
                return Err(BlobError::TooSmall);
            }
            V2_HEADER_SIZE
        }
        _ => return Err(BlobError::UnsupportedVersion),
    };

    let heading_count = read_u32_le(data, 16);
    let link_count = read_u32_le(data, 20);
    let tag_count = read_u32_le(data, 24);
    let block_id_count = read_u32_le(data, 28);
    let line_count = read_u32_le(data, 32);
    let text_pool_size = read_u32_le(data, 36);
    let total_blob_size = read_u32_le(data, 44);
    // code_span_count lives at offset 48 for both v1 and v2.
    let code_span_count = read_u32_le(data, 48);
    // task_count at offset 56, embed_count at offset 52, callout_count at offset 60,
    // block_ref_count at offset 72 (v2 only; v1 → 0).
    let (
        embed_count,
        task_count,
        callout_count,
        block_ref_count,
        query_block_count,
        link_def_count,
        property_count,
        xml_tag_count,
    ) = if version >= BLOB_VERSION_V2 {
        (
            read_u32_le(data, 52),
            read_u32_le(data, 56),
            read_u32_le(data, 60),
            read_u32_le(data, 72),
            read_u32_le(data, 64),
            read_u32_le(data, 68),
            read_u32_le(data, 76),
            read_u32_le(data, 80),
        )
    } else {
        (0, 0, 0, 0, 0, 0, 0, 0)
    };

    // Compute expected total size via checked arithmetic to prevent overflow.
    let expected = header_size
        .checked_add(
            (heading_count as usize)
                .checked_mul(HEADING_SIZE)
                .ok_or(BlobError::SizeMismatch)?,
        )
        .and_then(|s| s.checked_add((link_count as usize).checked_mul(LINK_SIZE)?))
        .and_then(|s| s.checked_add((tag_count as usize).checked_mul(TAG_SIZE)?))
        .and_then(|s| s.checked_add((block_id_count as usize).checked_mul(BLOCK_ID_SIZE)?))
        .and_then(|s| s.checked_add((code_span_count as usize).checked_mul(CODE_SPAN_SIZE)?))
        .and_then(|s| s.checked_add((task_count as usize).checked_mul(TASK_SIZE)?))
        .and_then(|s| s.checked_add((embed_count as usize).checked_mul(EMBED_SIZE)?))
        .and_then(|s| s.checked_add((callout_count as usize).checked_mul(CALLOUT_SIZE)?))
        .and_then(|s| s.checked_add((block_ref_count as usize).checked_mul(BLOCK_REF_SIZE)?))
        .and_then(|s| s.checked_add((query_block_count as usize).checked_mul(QUERY_BLOCK_SIZE)?))
        .and_then(|s| s.checked_add((link_def_count as usize).checked_mul(LINK_DEF_SIZE)?))
        .and_then(|s| s.checked_add((property_count as usize).checked_mul(PROPERTY_SIZE)?))
        .and_then(|s| s.checked_add((xml_tag_count as usize).checked_mul(XML_TAG_SIZE)?))
        .and_then(|s| s.checked_add((line_count as usize).checked_mul(4)?))
        .and_then(|s| s.checked_add(text_pool_size as usize))
        .ok_or(BlobError::SizeMismatch)?;

    if expected != total_blob_size as usize || expected != data.len() {
        return Err(BlobError::SizeMismatch);
    }

    Ok(BlobHeader {
        header_size,
        heading_count,
        link_count,
        tag_count,
        block_id_count,
        code_span_count,
        task_count,
        embed_count,
        callout_count,
        block_ref_count,
        query_block_count,
        link_def_count,
        property_count,
        xml_tag_count,
        line_count,
        text_pool_size,
    })
}

// ---------------------------------------------------------------------------
// Section offsets
// ---------------------------------------------------------------------------

pub(super) struct SectionOffsets {
    pub(super) headings: usize,
    pub(super) links: usize,
    pub(super) tags: usize,
    pub(super) block_ids: usize,
    pub(super) code_spans: usize,
    pub(super) tasks: usize,
    pub(super) embeds: usize,
    pub(super) callouts: usize,
    pub(super) block_refs: usize,
    pub(super) query_blocks: usize,
    pub(super) link_definitions: usize,
    pub(super) properties: usize,
    pub(super) xml_tags: usize,
    // line_starts skipped — positions are pre-computed in the blob
    pub(super) text_pool: usize,
}

pub(super) fn compute_offsets(h: &BlobHeader) -> SectionOffsets {
    let headings = h.header_size;
    let links = headings + h.heading_count as usize * HEADING_SIZE;
    let tags = links + h.link_count as usize * LINK_SIZE;
    let block_ids = tags + h.tag_count as usize * TAG_SIZE;
    let code_spans = block_ids + h.block_id_count as usize * BLOCK_ID_SIZE;
    let tasks = code_spans + h.code_span_count as usize * CODE_SPAN_SIZE;
    let embeds = tasks + h.task_count as usize * TASK_SIZE;
    let callouts = embeds + h.embed_count as usize * EMBED_SIZE;
    let block_refs = callouts + h.callout_count as usize * CALLOUT_SIZE;
    let query_blocks = block_refs + h.block_ref_count as usize * BLOCK_REF_SIZE;
    let link_definitions = query_blocks + h.query_block_count as usize * QUERY_BLOCK_SIZE;
    let properties = link_definitions + h.link_def_count as usize * LINK_DEF_SIZE;
    let xml_tags = properties + h.property_count as usize * PROPERTY_SIZE;
    let line_starts = xml_tags + h.xml_tag_count as usize * XML_TAG_SIZE;
    let text_pool = line_starts + h.line_count as usize * 4;
    SectionOffsets {
        headings,
        links,
        tags,
        block_ids,
        code_spans,
        tasks,
        embeds,
        callouts,
        block_refs,
        query_blocks,
        link_definitions,
        properties,
        xml_tags,
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
pub(super) fn pool_str(text_pool: &[u8], off: u32, len: u32) -> Result<&str, BlobError> {
    let start = off as usize;
    let end = start
        .checked_add(len as usize)
        .ok_or(BlobError::TextPoolOutOfBounds)?;
    if end > text_pool.len() {
        return Err(BlobError::TextPoolOutOfBounds);
    }
    std::str::from_utf8(&text_pool[start..end]).map_err(|_| BlobError::InvalidUtf8)
}
