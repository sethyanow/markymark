---
id: marky-2u6h
title: 'Phase B-1: Blob v2 header expansion'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-ix3
---


Expand blob header from 64 to 128 bytes with count fields for all Phase B extraction types. Bump BLOB_VERSION from 1 to 2. Maintain backward compatibility (from_blob reads both v1 and v2).

## Context
Phase B adds 8 new extraction types to the Zig pipeline. The current blob header has only 3 reserved u32 slots (12 bytes). Need at least 8 new count fields plus reserved space for future expansion.

## Current Header Layout (v1, 64 bytes)
magic(u32), version(u16), flags(u16), content_hash(u64),
heading_count(u32), link_count(u32), tag_count(u32), block_id_count(u32),
line_count(u32), text_pool_size(u32), token_estimate(u32), total_blob_size(u32),
code_span_count(u32), _reserved[3](u32×3)

## New Header Layout (v2, 128 bytes)
Keep all v1 fields in place. Replace _reserved[3] and extend:
- embed_count(u32)
- task_count(u32)
- callout_count(u32)
- query_block_count(u32)
- link_def_count(u32)
- block_ref_count(u32)
- property_count(u32)
- xml_tag_count(u32)
- _reserved[N](u32×N) — fill remaining space to 128 bytes

## Files to Modify
- zig/src/engine/blob.zig: ScanBlobHeader, computeBlobSize, computeSectionOffsets, writeHeader, readHeader
- markymark-index/src/document/from_blob.rs: HEADER_SIZE, BlobHeader, validate_blob, compute_offsets, SectionOffsets
- zig/src/engine/document.zig: serializeState (write new count fields as 0 initially)

## Success Criteria
- [ ] BLOB_VERSION = 2 in both Zig and Rust
- [ ] Header is exactly 128 bytes in both Zig and Rust (comptime/const assert)
- [ ] All 8 new count fields present in header struct
- [ ] from_blob reads v1 blobs (code_span_count from reserved, new counts = 0)
- [ ] from_blob reads v2 blobs with new count fields
- [ ] computeBlobSize handles all new section sizes
- [ ] computeSectionOffsets includes new section offsets
- [ ] Existing tests pass (v1 backward compat)
- [ ] New tests: v2 blob roundtrip, v1→v2 migration, v2 header size assert
- [ ] No BlobFoo structs yet (just header + offsets — B-3..B-7 add the data structs)

## Design

## Goal
Expand blob header from 64 to 128 bytes. Bump BLOB_VERSION 1→2. Backward-compatible: from_blob reads both v1 and v2 blobs. All new count fields default to 0. No BlobFoo data structs yet (B-3..B-7 add those incrementally).

## Effort Estimate
4-6 hours

## Exact v2 Header Layout (128 bytes)

Offset  Field               Size  Notes
0       magic               4     unchanged (0x4D4B5343)
4       version             2     = 2 (was 1)
6       flags               2     unchanged
8       content_hash        8     unchanged
16      heading_count       4     unchanged
20      link_count          4     unchanged
24      tag_count           4     unchanged
28      block_id_count      4     unchanged
32      line_count          4     unchanged
36      text_pool_size      4     unchanged
40      token_estimate      4     unchanged
44      total_blob_size     4     unchanged (value increases by 64 due to larger header)
48      code_span_count     4     unchanged
52      embed_count         4     NEW (was _reserved[0..4])
56      task_count          4     NEW (was _reserved[4..8])
60      callout_count       4     NEW (was _reserved[8..12])
64      query_block_count   4     NEW (expansion start beyond old 64-byte boundary)
68      link_def_count      4     NEW
72      block_ref_count     4     NEW
76      property_count      4     NEW
80      xml_tag_count       4     NEW
84      _reserved_v2        44    NEW (11 × u32 = generous future expansion, no v3 needed)

IMPORTANT: Zig _reserved field is [12]u8 (bytes), NOT [3]u32.
New v2 _reserved_v2 should also be byte array: [44]u8.

## Section Ordering (unchanged for B-1; B-3..B-7 append incrementally)

Header (128 bytes for v2, 64 for v1)
BlobHeading[] × heading_count     (40 bytes each)
BlobLink[]    × link_count        (40 bytes each)
BlobTag[]     × tag_count         (24 bytes each)
BlobBlockId[] × block_id_count    (28 bytes each)
BlobCodeSpan[] × code_span_count  (32 bytes each)
--- future sections appended here by B-3..B-7, always BEFORE line_starts ---
line_starts[] × line_count        (4 bytes each)
text_pool     (text_pool_size bytes)

B-3..B-7 each add their section between code_spans and line_starts.
Section order MUST match between Zig serializeState and Rust from_blob.
Order is defined by the header field order: embed→task→callout→query_block→link_def→block_ref→property→xml_tag.

## Implementation Steps

### Step 1: RED — Write failing tests

Zig (blob.zig tests):
- test_v2_header_size_128: comptime assert @sizeOf(ScanBlobHeader) == 128
- test_computeBlobSize_v2_empty: computeBlobSize(0,0,0,0,0,0,0,0,0,0,0,0,0,0) returns 128 (header only)
- test_v2_header_field_offsets: verify embed_count at byte 52, task_count at 56, etc.

Rust (from_blob.rs tests):
- test_from_blob_v1_backward_compat: existing v1 blob (64-byte header) still parses correctly
- test_from_blob_v2_empty: v2 blob (128-byte header, all counts=0) parses to empty document
- test_from_blob_rejects_truncated_v2: version=2 with <128 bytes → TooSmall
- test_from_blob_v2_same_result_as_v1: v2 blob with zero new counts produces identical DocumentIndex to v1

### Step 2: GREEN — Zig side

a. blob.zig: Update ScanBlobHeader extern struct
   - Replace _reserved: [12]u8 with:
     embed_count: u32 = 0, task_count: u32 = 0, callout_count: u32 = 0,
     query_block_count: u32 = 0, link_def_count: u32 = 0, block_ref_count: u32 = 0,
     property_count: u32 = 0, xml_tag_count: u32 = 0, _reserved_v2: [44]u8 = .{0} ** 44
   - Update BLOB_VERSION to 2
   - Update comptime size assertion: @sizeOf(ScanBlobHeader) == 128

b. blob.zig: Update computeBlobSize
   - Add header_size parameter or hardcode 128
   - For B-1: new section sizes are 0 (no BlobFoo structs defined yet)
   - Function must accept all new count params (even though they'll be 0)
   - Signature grows: add embed_count, task_count, callout_count, query_block_count,
     link_def_count, block_ref_count, property_count, xml_tag_count params

c. blob.zig: Update computeSectionOffsets
   - No new offsets yet (counts are 0, no struct sizes defined)
   - BUT must account for header growing from 64→128 (offsets shift by +64)
   - This affects ALL existing section offsets

d. blob.zig: Update writeHeader/readHeader for new 128-byte layout

e. document.zig: Update serializeState
   - Write 128-byte header instead of 64-byte
   - Set all new count fields to 0
   - total_blob_size increases by 64 for every blob

### Step 3: GREEN — Rust side

a. from_blob.rs: Update constants
   - BLOB_VERSION remains 1 as minimum, add BLOB_VERSION_2 = 2
   - Add V1_HEADER_SIZE = 64, V2_HEADER_SIZE = 128
   - HEADER_SIZE = V2_HEADER_SIZE (for new blobs)

b. from_blob.rs: Update BlobHeader struct
   - Add: embed_count, task_count, callout_count, query_block_count,
     link_def_count, block_ref_count, property_count, xml_tag_count

c. from_blob.rs: Update validate_blob
   - Read version at offset 4
   - If version == 1: require data.len() >= 64, parse 64-byte header,
     set all new count fields to 0 (backward compat)
   - If version == 2: require data.len() >= 128, parse full 128-byte header
   - If version > 2: return Err(UnsupportedVersion)
   - Update UnsupportedVersion message: "only versions 1 and 2 are supported"
   - Update TooSmall message: version-aware minimum size

d. from_blob.rs: Update compute_offsets
   - header_size is version-dependent: v1=64, v2=128
   - Section offsets start at header_size (not hardcoded 64)
   - No new section offsets yet (counts are 0)

e. from_blob.rs: Update BlobError Display
   - TooSmall: "blob too small (minimum 64 bytes for v1, 128 bytes for v2)"
   - UnsupportedVersion: "unsupported blob version (versions 1 and 2 supported)"

f. from_blob.rs: Update existing golden blob test
   - Golden blob helper (blob_for) must produce v2 blobs (128-byte header)
   - All manually-constructed blob byte arrays in tests must be updated
   - Parity test (test_from_blob_parity_with_from_scan) must still pass

### Step 4: REFACTOR
- cargo test -p markymark-index (all blob tests green)
- cargo clippy --workspace --all-targets
- Zig tests: zig build test in zig/

## Success Criteria
- [ ] ScanBlobHeader is exactly 128 bytes (Zig comptime assert)
- [ ] Rust V2_HEADER_SIZE const == 128 (const assert)
- [ ] All 8 new count fields present and defaulting to 0
- [ ] BLOB_VERSION = 2 in Zig, validate_blob accepts versions 1 and 2 in Rust
- [ ] v1 blob (64-byte header) still parses: test_from_blob_v1_backward_compat passes
- [ ] v2 blob (128-byte header) parses: test_from_blob_v2_empty passes
- [ ] Truncated v2 blob rejected: test_from_blob_rejects_truncated_v2 passes
- [ ] v2 with zero new counts == v1 behavior: test_from_blob_v2_same_result_as_v1 passes
- [ ] Existing golden blob roundtrip still passes (updated for 128-byte header)
- [ ] Existing parity test (from_blob vs from_scan) still passes
- [ ] Section offsets correct: existing tests pass (offsets shifted by +64)
- [ ] cargo clippy --workspace --all-targets clean
- [ ] Zig blob tests pass (header size, computeBlobSize, field offsets)

## Anti-Patterns (FORBIDDEN)
- ❌ No hardcoded 64 remaining for header size — use V1_HEADER_SIZE/V2_HEADER_SIZE constants
- ❌ No unwrap/expect in validate_blob or from_blob (already using Result, maintain)
- ❌ No changing section ordering for existing types (headings→links→tags→block_ids→code_spans→line_starts→text_pool)
- ❌ No BlobFoo data structs in B-1 (those belong in B-3..B-7)
- ❌ No breaking the from_blob public API (same function signatures, just accepts both versions)

## Key Considerations (SRE Review)

**Edge Case: total_blob_size increases by 64 for ALL v2 blobs**
Every blob produced after B-1 has a 128-byte header instead of 64.
total_blob_size field increases by 64. This means:
- All manually-constructed test blobs need header size update
- blob_for() helper in tests needs updating
- Golden blob test fixture needs regeneration
Action: Update blob_for() first, then fix each test.

**Edge Case: v1 blob with code_span_count in _reserved[0]**
Current v1 blobs have code_span_count at offset 48 (a named field).
The _reserved bytes at offset 52 may contain nonzero data from future
bug or memory corruption. When parsing v1, read code_span_count at 48
but do NOT read new count fields from _reserved (set to 0).

**Edge Case: Section offset shift**
compute_offsets starts sections at header_size. Changing from 64→128
shifts ALL section offsets by +64 for v2 blobs. For v1 blobs, offsets
start at 64 (unchanged). This MUST be tested with the parity test.

**Edge Case: Zig computeBlobSize signature change**
Adding 8 new params to computeBlobSize is unwieldy. Consider:
- Option A: Pass ScanBlobHeader struct directly (has all counts)
- Option B: Keep individual params (matches current style)
Prefer Option A if it's cleaner. Both work.

**Edge Case: computeSectionOffsets for v1 vs v2**
Rust compute_offsets must accept header_size parameter (or derive from version).
Section offsets are header_size-dependent. Currently hardcoded as
HEADER_SIZE + count × size. Must parameterize.

**Reference Implementation**
- Zig header: zig/src/engine/blob.zig lines 24-39 (ScanBlobHeader)
- Rust header: markymark-index/src/document/from_blob.rs lines 122-130 (BlobHeader)
- Rust validation: from_blob.rs lines 132-197 (validate_blob)
- Rust offsets: from_blob.rs lines 199-230 (compute_offsets)
- Zig serialization: zig/src/engine/document.zig (serializeState)
