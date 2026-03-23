---
id: marky-6jzs
title: 'Task 1: Zig DocumentEngine struct + blob serialization format'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-io3h
---



## Design

## Goal

Implement the Zig-side DocumentEngine struct and blob serialization format. This is the foundation — Zig owns document state and can serialize it to a flat binary blob. No FFI exports yet (that's task 2).

## Effort Estimate

12-16 hours. Blob format and engine are tightly coupled so kept as one task with clear checkpoints: (1) extern struct definitions + validate, (2) serialize, (3) engine create/destroy, (4) engine update + getBlob.

## Implementation

### 1. Study existing patterns
- zig/src/md4c/extraction_renderer.zig: ExtractionRenderer.render() is the entry point. Call this directly from engine create/update to get headings + links.
- zig/src/md4c/parser.zig: MdParser.init() + parse(renderer) is the parse entry. Engine creates its own MdParser instance.
- zig/src/shared/slug.zig: slugify() function. Call per heading. Dedup slugs by appending -1, -2 etc. (see Rust slug_to_heading pattern in markymark-index/src/document/mod.rs).
- zig/src/shared/kernels/tags.zig: scanTags() function for tag extraction. Call directly from Zig (no FFI needed, these are Zig functions).
- zig/src/shared/kernels/block_ids.zig: scanBlockIds() function for block ID extraction. Same — direct Zig call.
- zig/src/shared/tokens.zig: estimateTokens() for token count.
- zig/src/shared/c_adapter.zig: contentHash() for FNV-1a hash. buildFenceMap() for code block ranges (needed to filter tags/blocks).

### 2. Allocator strategy
- Engine internals use std.heap.GeneralPurposeAllocator in tests (leak detection) and std.heap.page_allocator in production (via comptime switch or allocator parameter).
- Engine.create() accepts an allocator parameter. Stored in engine for all internal allocations.
- On update(): free all old StoredHeading/Link/Tag/BlockId slices (individually, since each text/slug is a separate allocation), then rebuild.
- On destroy(): free all stored slices + cached blob if present.
- getBlob() allocates the blob with the engine's allocator. Blob is owned by engine and freed on next update() or destroy().

### 3. Slug dedup algorithm
- Maintain a temporary HashMap([]const u8, u32) during heading processing.
- For each heading, slugify the text.
- If slug not in map: insert with count=1, use slug as-is.
- If slug in map: increment count, append "-{count}" (e.g., heading → heading, heading-1, heading-2).
- This matches the Rust behavior in markymark-index/src/document/mod.rs from_scan() heading loop.

### 4. Write tests first (TDD)

#### Extraction correctness
- test_create_simple_markdown: Input "# Hello\n\nSome [link](url.md) text with #tag and ^blockid\n". Verify heading_count=1, link_count=1, tag_count=1, block_id_count=1. Verify heading text="Hello", level=1, slug="hello".
- test_create_multiple_headings: Input with 3 headings at levels 1, 2, 3. Verify counts and levels.
- test_entity_decoding: Input "# Hello &amp; World\n". Verify heading text="Hello & World" (entity decoded).
- test_wiki_links: Input "See [[Other Page]] and [normal](link.md)\n". Verify link_count=2, is_wiki flags correct.

#### Slug dedup
- test_slug_dedup: Input "# Title\n\n# Title\n\n# Title\n". Verify slugs are ["title", "title-1", "title-2"].

#### Line starts and positions
- test_line_starts: Input "first\nsecond\nthird\n". Verify line_starts = [0, 6, 13, 19].
- test_byte_offset_to_position: Offset 0 → (0,0). Offset 6 → (1,0). Offset 8 → (1,2).

#### Blob serialization
- test_blob_header: Create engine → getBlob(). Verify first 4 bytes = 0x4D4B5343 ("MKSC"), version=1, counts match engine state.
- test_blob_text_pool: Create engine with known heading "Hello". Verify text_pool region of blob contains "Hello" and the heading's text_off/text_len point to it.
- test_blob_empty_document: Input "" (empty). Verify blob is header (64 bytes) only, all counts=0.
- test_blob_validate_rejects_bad_magic: Construct a blob with wrong magic. validateBlob() returns error.

#### Update
- test_update_replaces_state: Create with "# A\n". Update with "# B\n". Verify heading text changed to "B".
- test_update_invalidates_blob: Create → getBlob(). Update with new text. getBlob() returns new blob (not stale cached one).
- test_update_changes_counts: Create with 1 heading. Update with 3 headings. Verify heading_count=3.

#### Memory safety
- test_create_destroy_no_leaks: Use GPA, create engine, destroy. Verify gpa.deinit() returns .ok.
- test_update_100_times_no_leaks: Use GPA, create, update 100 times with varying markdown, destroy. Verify .ok.

### 5. Implementation checklist
- [ ] zig/src/engine/blob.zig — ScanBlobHeader extern struct (64 bytes, magic=0x4D4B5343, version=1)
- [ ] zig/src/engine/blob.zig — BlobHeading extern struct (40 bytes: text_off/len, slug_off/len, source_offset, start_line/col, end_line/col, level, padding)
- [ ] zig/src/engine/blob.zig — BlobLink extern struct (40 bytes: text_off/len, target_off/len, source_offset, start_line/col, end_line/col, is_wiki, padding)
- [ ] zig/src/engine/blob.zig — BlobTag extern struct (24 bytes: name_off/len, source_offset, start_line/col, padding)
- [ ] zig/src/engine/blob.zig — BlobBlockId extern struct (28 bytes: id_off/len, source_offset, start_line/col, end_line/col)
- [ ] zig/src/engine/blob.zig — comptime size assertions (@sizeOf checks for all 5 structs)
- [ ] zig/src/engine/blob.zig — validateBlob(data: []const u8) → validates magic, version, size bounds, offset consistency
- [ ] zig/src/engine/blob.zig — serializeState() → packs StoredHeading/Link/Tag/BlockId arrays + line_starts + text pool into contiguous []u8
- [ ] zig/src/engine/document.zig — StoredHeading struct (text: []const u8, slug: []const u8, source_offset: u32, start_line/col/end_line/col: u32, level: u8)
- [ ] zig/src/engine/document.zig — StoredLink, StoredTag, StoredBlockId structs (similar pattern)
- [ ] zig/src/engine/document.zig — DocumentEngine struct with allocator, stored arrays, line_starts, token_estimate, content_hash, cached_blob
- [ ] zig/src/engine/document.zig — create(text, allocator) → parse md4c, run SIMD scans, compute line_starts, slugify+dedup, convert positions
- [ ] zig/src/engine/document.zig — update(text) → free old state, full reparse, replace state, set cached_blob = null
- [ ] zig/src/engine/document.zig — getBlob() → if cached_blob null, call serializeState(); return cached_blob
- [ ] zig/src/engine/document.zig — destroy() → free all owned slices, free cached_blob, free engine struct
- [ ] zig/src/engine/document.zig — helper: computeLineStarts(text) → []u32
- [ ] zig/src/engine/document.zig — helper: byteOffsetToPosition(line_starts, offset) → struct{line: u32, col: u32}
- [ ] zig/build.zig — add engine module to build, wire test step

## Success Criteria
- [ ] All Zig tests pass: zig build test (18+ tests listed above)
- [ ] GPA leak detection passes for create/destroy and create/update×100/destroy
- [ ] Blob header validates: magic=0x4D4B5343, version=1, counts match
- [ ] Blob for empty document is exactly 64 bytes (header only, all counts 0)
- [ ] Blob text pool contains all strings (heading text+slug, link text+target, tag name, block ID)
- [ ] Heading slugs deduplicated correctly: "title", "title-1", "title-2" pattern
- [ ] Positions match manual calculation for known offsets (specific test vectors, not cross-language)
- [ ] Entity references decoded in headings and link text (e.g. &amp; → &)
- [ ] Zero compiler warnings (zig build test must compile clean)
- [ ] comptime @sizeOf assertions for all 5 blob structs

## Anti-Patterns (FORBIDDEN)

- ❌ NO std.heap.page_allocator for per-element allocations inside parse loop (performance: use arena or batch allocator for extraction intermediates, page_allocator only for final stored results)
- ❌ NO @ptrCast without alignment validation first (safety: misaligned pointers panic in Debug/ReleaseSafe, see marky-5rq lesson)
- ❌ NO bare @panic or unreachable in any code path (safety: return errors, let caller handle)
- ❌ NO modifying ExtractionRenderer (scope: call it as-is, don't fork. Future optimization of extraction is a separate task)
- ❌ NO text pool offsets exceeding u32 range without checked arithmetic (safety: document >4GB would overflow)
- ❌ NO leaked allocations on error paths (safety: use defer for cleanup, test with GPA)

## Key Considerations (SRE Review)

**Edge Case: Empty document**
- Input "" must produce valid blob with header only (64 bytes), all counts = 0
- Engine state must have 0 headings, links, tags, blocks
- line_starts should be [0] (one line with zero offset)
- Test: test_blob_empty_document

**Edge Case: Unicode in heading text**
- Headings can contain emoji, CJK, RTL text
- Slugify must handle UTF-8 correctly (existing slug.zig handles this)
- Blob text pool stores raw UTF-8 bytes, text_len is byte length not char count
- Test: add a Unicode heading test if not covered by existing extraction_renderer tests

**Edge Case: u32 overflow for very large documents**
- text_pool_size, source_offset, text_off are all u32 → max 4GB
- For documents >4GB, offsets would overflow silently
- Mitigation: checked arithmetic in blob packing. If total text pool > maxInt(u32), return error.OutOfRange
- Practical risk: low (markdown files rarely >1MB), but must not silently corrupt

**Edge Case: md4c parse failure**
- ExtractionRenderer.render() can return error (e.g., malformed input triggers md4c internal error)
- create() and update() must propagate this error, not @panic
- On update() error: keep old state (don't free old before confirming new parse succeeds)
- Test: consider whether md4c can actually fail on any valid UTF-8 input (likely very rare)

**Edge Case: Heading with empty text**
- Input "## \n" (heading with only whitespace)
- Slugify produces empty slug → must handle (empty string is valid slug)
- Dedup must handle empty slug collisions

**Error handling in update()**
- CRITICAL: Parse new text FIRST, then free old state, then replace
- If parse fails: return error, old state preserved
- Pattern: new_state = try parseAll(text); freeOldState(); self.state = new_state;
- This prevents data loss on transient parse failures

**SIMD scan integration**
- Tag and block_id scans need fence_map (code block ranges) to filter false positives
- Fence map must be computed first: buildFenceMap(text)
- Then scanTags(text, fence_ranges) and scanBlockIds(text, fence_ranges)
- These are Zig-internal calls, not FFI — call the kernel functions directly
