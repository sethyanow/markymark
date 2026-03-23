---
id: marky-2n4u
title: 'Task 3: Rust DocumentIndex::from_blob() constructor'
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-atsp, marky-0mr.4, marky-0mr.6, marky-0mr.9]
parent: marky-io3h
---







## Design

## Goal

Implement \`DocumentIndex::from_blob(blob: &[u8]) -> Result<Self, BlobError>\` that reads the Zig engine's binary blob format and produces a \`DocumentIndex\` identical to \`from_scan()\` for the same input document (except end positions — see Known Parity Gaps).

## Effort Estimate

4-6 hours implementation + testing.

## Context

Tasks 1-2 delivered:
- Zig DocumentEngine with create/update/getBlob/destroy (marky-6jzs)
- FFI exports + Rust DocumentEngine wrapper with ScanBlob type (marky-atsp)

The blob format (defined in zig/src/engine/blob.zig) is:
\`\`\`
[ScanBlobHeader: 64 bytes]  magic(4) version(2) flags(2) content_hash(8) heading_count(4) link_count(4) tag_count(4) block_id_count(4) line_count(4) text_pool_size(4) token_estimate(4) total_blob_size(4) _reserved(16)
[BlobHeading[N]: 40 bytes each]  text_off/len, slug_off/len, source_offset, start_line/col, end_line/col, level
[BlobLink[N]: 40 bytes each]  text_off/len, target_off/len, source_offset, start_line/col, end_line/col, is_wiki
[BlobTag[N]: 24 bytes each]  name_off/len, source_offset, start_line/col
[BlobBlockId[N]: 28 bytes each]  id_off/len, source_offset, start_line/col, end_line/col
[u32 x line_count]  (line_starts - not needed by from_blob, positions pre-computed)
[u8 x text_pool_size]  (contiguous text pool)
\`\`\`

## Implementation

### File: markymark-index/src/document/from_blob.rs (new)

#### Step 1: BlobError enum and constants

\`\`\`rust
use thiserror::Error;  // or manual impl Display+Error if thiserror not in deps

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobError {
    TooSmall,
    InvalidMagic,
    UnsupportedVersion,
    SizeMismatch,
    TextPoolOutOfBounds,
    InvalidUtf8,
}

const BLOB_MAGIC: u32 = 0x4D4B_5343; // "MKSC"
const BLOB_VERSION: u16 = 1;
const HEADER_SIZE: usize = 64;
const HEADING_SIZE: usize = 40;
const LINK_SIZE: usize = 40;
const TAG_SIZE: usize = 24;
const BLOCK_ID_SIZE: usize = 28;
\`\`\`

#### Step 2: Header parsing and validation

\`\`\`rust
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]])
}
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset+1]])
}
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset+8].try_into().unwrap())
}

struct BlobHeader {
    heading_count: u32,
    link_count: u32,
    tag_count: u32,
    block_id_count: u32,
    line_count: u32,
    text_pool_size: u32,
    token_estimate: u32,
    total_blob_size: u32,
    content_hash: u64,
}

fn validate_blob(data: &[u8]) -> Result<BlobHeader, BlobError> {
    if data.len < HEADER_SIZE { return Err(BlobError::TooSmall); }
    let magic = read_u32_le(data, 0);
    if magic != BLOB_MAGIC { return Err(BlobError::InvalidMagic); }
    let version = read_u16_le(data, 4);
    if version != BLOB_VERSION { return Err(BlobError::UnsupportedVersion); }
    // Parse counts from header (offsets match Zig extern struct layout)
    let content_hash = read_u64_le(data, 8);
    let heading_count = read_u32_le(data, 16);
    let link_count = read_u32_le(data, 20);
    let tag_count = read_u32_le(data, 24);
    let block_id_count = read_u32_le(data, 28);
    let line_count = read_u32_le(data, 32);
    let text_pool_size = read_u32_le(data, 36);
    let token_estimate = read_u32_le(data, 40);
    let total_blob_size = read_u32_le(data, 44);
    // Compute expected size using checked arithmetic
    let expected = HEADER_SIZE
        .checked_add((heading_count as usize).checked_mul(HEADING_SIZE).ok_or(BlobError::SizeMismatch)?)
        .and_then(|s| s.checked_add((link_count as usize).checked_mul(LINK_SIZE)?))
        .and_then(|s| s.checked_add((tag_count as usize).checked_mul(TAG_SIZE)?))
        .and_then(|s| s.checked_add((block_id_count as usize).checked_mul(BLOCK_ID_SIZE)?))
        .and_then(|s| s.checked_add((line_count as usize).checked_mul(4)?))
        .and_then(|s| s.checked_add(text_pool_size as usize))
        .ok_or(BlobError::SizeMismatch)?;
    if expected != total_blob_size as usize || expected != data.len {
        return Err(BlobError::SizeMismatch);
    }
    Ok(BlobHeader { heading_count, link_count, tag_count, block_id_count, line_count, text_pool_size, token_estimate, total_blob_size, content_hash })
}
\`\`\`

#### Step 3: Section offset computation

\`\`\`rust
struct SectionOffsets {
    headings: usize,
    links: usize,
    tags: usize,
    block_ids: usize,
    line_starts: usize,
    text_pool: usize,
}

fn compute_offsets(h: &BlobHeader) -> SectionOffsets {
    let headings = HEADER_SIZE;
    let links = headings + h.heading_count as usize * HEADING_SIZE;
    let tags = links + h.link_count as usize * LINK_SIZE;
    let block_ids = tags + h.tag_count as usize * TAG_SIZE;
    let line_starts = block_ids + h.block_id_count as usize * BLOCK_ID_SIZE;
    let text_pool = line_starts + h.line_count as usize * 4;
    SectionOffsets { headings, links, tags, block_ids, line_starts, text_pool }
}
\`\`\`

#### Step 4: text_pool string extraction helper

\`\`\`rust
fn pool_str<'a>(text_pool: &'a [u8], off: u32, len: u32) -> Result<&'a str, BlobError> {
    let start = off as usize;
    let end = start.checked_add(len as usize).ok_or(BlobError::TextPoolOutOfBounds)?;
    if end > text_pool.len() { return Err(BlobError::TextPoolOutOfBounds); }
    std::str::from_utf8(&text_pool[start..end]).map_err(|_| BlobError::InvalidUtf8)
}
\`\`\`

#### Step 5: DocumentIndex::from_blob() main function

Build the DocumentIndex using the same self_cell pattern as from_ast/from_scan:
- Create fresh DocumentArena
- Parse all entries from blob using section offsets
- For each BlobHeading: arena_alloc_str(text, slug), build HeadingEntry with positions from blob (start_line/col, end_line/col)
- For each BlobLink where is_wiki==1: target = pool text at target_off/len, alias = (text != target ? Some(text) : None), heading = None. Build WikiLinkEntry
- For each BlobLink where is_wiki==0: text = pool text, split target on '#' for url + anchor (matching from_scan). Build MarkdownLinkEntry
- For each BlobTag: name = pool text at name_off/len. Build TagEntry
- For each BlobBlockId: id = pool text at id_off/len. Build BlockEntry with source_offset as start_byte, end_byte = source_offset + 1 + id_len (the +1 accounts for the '^' prefix)
- Build TOC and outline via helpers::build_toc/build_outline
- Set xml_tags, frontmatter, aliases, properties, block_refs to empty (same as from_scan)
- Return DocumentIndex

### File: markymark-index/src/document/mod.rs (modified)

Add \`mod from_blob;\` and feature-gate it with \`#[cfg(feature = \"zig-kernels\")]\` since it depends on the engine FFI.

## Known Parity Gaps (v1 Accepted)

**End positions differ**: Zig engine sets \`end = start\` for all headings, links, and block_ids (see document.zig:243,260 — explicit comment: \"Same as start for v1\"). The from_scan path computes end positions from byte offsets. The parity test MUST only compare: text, slug, level, start position, link type, tag names, block IDs. End positions are NOT compared.

**Wiki link heading field**: from_blob and from_scan both set \`heading: None\` for wiki links (md4c stores the full target including any \`#section\` as part of the target string, same as from_scan's SIMD scanner).

## Anti-Patterns (FORBIDDEN)

- NO unsafe pointer casts for struct reading (use byte-level reads with from_le_bytes for alignment safety)
- NO unwrap() or expect() in production code paths (all fallible ops return Result<_, BlobError>)
- NO panic paths in from_blob (corrupted blob = Err, never panic)
- NO TODO or stub implementations
- NO allocations outside the arena for per-entry data (text_pool strings go into arena_alloc_str)
- NO skipping text_pool bounds validation (every text_off + text_len must be checked against text_pool_size)

## Tests (feature-gated #[cfg(feature = \"zig-kernels\")])

Each test uses the real DocumentEngine FFI to produce blobs (NOT hand-crafted blobs), ensuring end-to-end correctness.

1. **test_from_blob_empty_document**: Engine with \"\" → from_blob → empty headings, links, tags, blocks. Bug caught: off-by-one in empty blob handling.

2. **test_from_blob_single_heading**: Engine with \"# Hello\\n\" → from_blob → 1 heading with text=\"Hello\", slug=\"hello\", level=1, correct start position. Bug caught: basic deserialization, text_pool offset math.

3. **test_from_blob_multiple_headings_with_dedup_slugs**: Engine with \"# Title\\n\\n# Title\\n\" → from_blob → 2 headings, slugs \"title\" and \"title-1\". Bug caught: slug dedup via Zig preserved through blob.

4. **test_from_blob_wiki_link**: Engine with \"[[My Page]]\\n\" → from_blob → 1 wiki link with target=\"My Page\", alias=None. Bug caught: is_wiki flag parsing, alias detection.

5. **test_from_blob_wiki_link_with_alias**: Engine with \"[[target|display]]\\n\" → from_blob → 1 wiki link with target=\"target\", alias=Some(\"display\"). Bug caught: alias vs non-alias text comparison.

6. **test_from_blob_markdown_link_with_anchor**: Engine with \"[text](url.md#frag)\\n\" → from_blob → 1 markdown link with url=\"url.md\", anchor=Some(\"frag\"). Bug caught: '#' splitting logic.

7. **test_from_blob_tags**: Engine with \"text #alpha #beta\\n\" → from_blob → tags containing \"alpha\" and \"beta\". Bug caught: tag extraction from blob.

8. **test_from_blob_block_ids**: Engine with \"content ^my-id\\n\" → from_blob → block ID \"my-id\" present. Bug caught: block ID extraction, source_offset mapping.

9. **test_from_blob_toc_and_outline**: Engine with \"# A\\n\\n## B\\n\\n### C\\n\" → from_blob → TOC with depths [0,1,2], outline tree with correct nesting. Bug caught: TOC/outline construction from blob headings.

10. **test_from_blob_rejects_invalid_magic**: Hand-crafted 64-byte buffer with wrong magic → BlobError::InvalidMagic. Bug caught: validation bypass.

11. **test_from_blob_rejects_bad_version**: Hand-crafted buffer with magic OK but version=99 → BlobError::UnsupportedVersion. Bug caught: version check.

12. **test_from_blob_rejects_truncated**: 32-byte buffer → BlobError::TooSmall. Bug caught: buffer underflow.

13. **test_from_blob_rejects_size_mismatch**: Valid header but total_blob_size doesn't match actual data length → BlobError::SizeMismatch. Bug caught: size validation.

14. **test_from_blob_parity_with_from_scan**: For markdown text with headings + wiki links + markdown links + tags + block IDs, compare from_blob output vs from_scan output: heading text/slug/level MUST match exactly, wiki link targets/aliases MUST match, markdown link text/url/anchor MUST match, tag names MUST match, block IDs MUST match. Start positions NOT compared (different computation paths). Bug caught: semantic drift between from_blob and from_scan paths.

15. **test_from_blob_mixed_document**: Complex document with multiple headings, wiki links with and without aliases, markdown links with and without anchors, tags, block IDs. Bug caught: interaction between multiple entity types, text_pool offset accumulation.

## Key Considerations (SRE Review)

**Edge Case: Text pool bounds overflow**
- Every text_off + text_len MUST be bounds-checked against text_pool.len()
- Attacker-controlled blob could have text_off=0xFFFFFFFF → arithmetic overflow
- Use checked_add for all offset+length computations
- Return BlobError::TextPoolOutOfBounds on any violation

**Edge Case: Empty text_pool entries**
- Heading text can be empty (\"# \\n\" produces empty heading text with empty slug)
- Tag name length 0 should produce empty TagEntry (not skip)
- Block ID length 0 is invalid but should not panic

**Edge Case: Arithmetic overflow in size computation**
- heading_count * 40 can overflow u32 for large counts
- Use usize checked arithmetic for all size computations
- Return BlobError::SizeMismatch on overflow

**Edge Case: Non-UTF8 in text_pool**
- Zig engine guarantees UTF8 (md4c processes UTF8 input, SIMD scanners work on byte level but text comes from valid UTF8 input)
- However, corrupted blob could contain non-UTF8
- from_blob MUST validate with std::str::from_utf8 and return BlobError::InvalidUtf8

**Endianness**
- Blob uses little-endian (written by Zig @memcpy which preserves native byte order on LE platforms)
- Rust from_le_bytes handles this correctly
- Both aarch64 (macOS) and x86_64 are little-endian

**Block ID byte range computation**
- Blob stores source_offset (byte offset of the '^' character)
- start_byte = source_offset as usize
- end_byte = source_offset as usize + 1 + id_len as usize ('^' + id text)
- This matches how from_scan computes block ranges

**Reference implementations to study**
- from_scan() in markymark-index/src/document/mod.rs:547 (the most similar construction path)
- self_cell pattern with DocumentOwner/DocumentDependent (same arena ownership model)
- arena_ref() pattern for accessing bump allocator within self_cell closure

## Success Criteria

- [ ] from_blob(engine_blob) produces valid DocumentIndex with correct headings, links, tags, block IDs
- [ ] from_blob(engine_blob) heading text/slug/level matches from_scan for same input (parity test #14)
- [ ] from_blob(engine_blob) wiki link target/alias matches from_scan for same input
- [ ] from_blob(engine_blob) markdown link text/url/anchor matches from_scan for same input
- [ ] from_blob(engine_blob) tag names match from_scan for same input
- [ ] from_blob(engine_blob) block IDs match from_scan for same input
- [ ] Invalid blobs rejected with appropriate BlobError variant (tests #10-13)
- [ ] Text pool bounds checked for every string extraction (no panic on corrupt blob)
- [ ] All 15 tests pass: cargo nextest -p markymark-index
- [ ] All existing tests still pass: cargo nextest
- [ ] Zero clippy warnings: cargo clippy --workspace --all-targets
