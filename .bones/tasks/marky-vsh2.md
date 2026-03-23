---
id: marky-vsh2
title: 'Phase A-2: Wire code spans through engine blob, ScanBackend, from_scan, from_blob, DocumentIndex'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ix3, marky-pdyo]
---




## Design

## Goal
Wire code span extraction end-to-end from Zig engine blob through to DocumentIndex in Rust. Phase A-1 (marky-pdyo) built the bottom half (Zig ExtractionRenderer captures code spans, FFI types exist). This task connects the remaining pieces so code spans flow through the engine blob, from_blob, ScanBackend, from_scan, and into DocumentDependent.

## Effort Estimate
8-10 hours (sequential steps, each testable independently)

## Preconditions
- Phase A-1 (marky-pdyo) complete: ExtractedCodeSpan in extraction_renderer.zig, CMd4cCodeSpan/Md4cCodeSpan/CodeSpanEntry/CodeSpanOwned types all exist
- Zig ExtractionRenderer enterSpan/leaveSpan .code callbacks working (verified by 9 Zig + 4 Rust FFI tests)

## Implementation Steps

### Step 1: Zig — Add StoredCodeSpan + wire parseAll
File: zig/src/engine/document.zig

Add StoredCodeSpan struct (mirrors StoredHeading/StoredLink pattern):
\`\`\`zig
const StoredCodeSpan = struct {
    text: []const u8,       // owned decoded text
    source_offset: u32,     // byte offset of opening backtick
    end_offset: u32,        // byte offset past closing backtick
    start: Position,        // line:col of opening backtick
    end: Position,          // line:col past closing backtick
};
\`\`\`

Add to DocumentEngine struct: \`code_spans: []StoredCodeSpan = &.{}\`

Extend parseAll signature: add \`out_code_spans: *[]StoredCodeSpan\` parameter.

In parseAll body after step 5 (link processing), add step 5b:
- Iterate \`extraction.code_spans\`, compute start/end positions via byteOffsetToPosition
- Transfer text ownership from extraction into stored_code_spans_list
- Free extraction.code_spans slice container alongside extraction.headings/links (line 295-296)

Add freeCodeSpans and freeStoredCodeSpansList helpers (follow freeHeadings pattern).

Update freeState to free code_spans.

Update parseAndStore to pass &self.code_spans.

### Step 2: Zig — Add BlobCodeSpan + blob v2 header
File: zig/src/engine/blob.zig

Add BlobCodeSpan struct:
\`\`\`zig
pub const BlobCodeSpan = extern struct {
    text_off: u32 = 0,
    text_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
    _pad: u8 = 0,       // align to 4-byte boundary (total 28 bytes)
    _pad2: [3]u8 = .{0} ** 3,
};
\`\`\`

Update ScanBlobHeader:
- BLOB_VERSION stays 1 (NOT bumped — use _reserved bytes instead)
- Replace first 4 bytes of _reserved with \`code_span_count: u32 = 0\`
- Reduce _reserved from [16]u8 to [12]u8
- RATIONALE: v1 blobs have _reserved all zeros, so code_span_count==0 means "no code spans" — backward compatible without version bump. Only bump version when reserved space exhausted.

Update computeBlobSize: add code_span_count parameter.
Update SectionOffsets: add code_spans field.
Update computeSectionOffsets: add code_spans offset calculation.

### Step 3: Zig — Serialize code spans in serializeState
File: zig/src/engine/document.zig, fn serializeState

Add code_spans to:
- u32 overflow guard (line 483-487)
- text_pool_size loop (add code span text lengths)
- computeBlobSize call (add code_span_count arg)
- Header construction (set code_span_count)
- Struct writing loop (after block_ids, before line_starts): iterate engine.code_spans, write BlobCodeSpan structs, write text to text pool, advance text_pool_cursor

### Step 4: Rust — Update from_blob for code spans
File: markymark-index/src/document/from_blob.rs

Add const: \`CODE_SPAN_SIZE: usize = 32;\` (sizeof BlobCodeSpan = 28, round up to check)

Update BlobHeader: add \`code_span_count: u32\` field.

Update validate_blob: read code_span_count from offset 48 (first 4 bytes of what was _reserved). Validate total size includes code span section.

Update SectionOffsets: add \`code_spans: usize\` field.

Update compute_offsets: insert code_spans section between block_ids and text_pool.

In from_blob_with_xml_tags (the main constructor):
- After extracting blocks (BlockData), extract code spans:
  - Read each BlobCodeSpan struct (source_offset, end_offset/text_off, text_len, positions)
  - Look up text via pool_str
  - Build CodeSpanOwned { text, range, start_byte, end_byte }
- Pass code_spans into the self_cell closure

BACKWARD COMPATIBILITY: If code_span_count == 0, skip code span section entirely. v1 blobs had _reserved[0..4] as zeros, so code_span_count reads as 0. No version check needed.

### Step 5: Rust — Add code_spans to DocumentDependent + accessor
File: markymark-index/src/document/mod.rs

Add to DocumentDependent struct:
\`\`\`rust
code_spans: &'a [CodeSpanEntry<'a>],
\`\`\`

Add accessor to impl DocumentIndex:
\`\`\`rust
pub fn code_spans<'a>(&'a self) -> &'a [CodeSpanEntry<'a>] {
    self.cell.borrow_dependent().code_spans
}
\`\`\`

Update from_blob_with_xml_tags closure: build CodeSpanEntry slice from CodeSpanOwned data, allocate in arena. Follow the same pattern as wiki_links (lines 412-440 of mod.rs).

Update from_ast_with_overrides_opt: if overrides.code_spans is Some, use it. Otherwise empty slice (from_ast doesn't extract code spans yet — that's Phase A-3/B).

### Step 6: Rust — Add scan_code_spans to ScanBackend trait
File: markymark-core/src/scanner.rs

Add CodeSpanResult type:
\`\`\`rust
pub struct CodeSpanResult {
    pub text: String,
    pub offset: u32,
    pub end_offset: u32,
}
\`\`\`

Add to ScanAllResult:
\`\`\`rust
pub code_spans: Vec<CodeSpanResult>,
\`\`\`

Add to ScanBackend trait with default impl:
\`\`\`rust
fn scan_code_spans(&self, text: &str) -> Result<Vec<CodeSpanResult>, ScanError> {
    Ok(Vec::new())  // Default: no code spans (backward compat for ZigScanBackend)
}
\`\`\`

Update default scan_all to include code_spans from scan_code_spans.

Implement scan_code_spans for Md4cScanBackend: call extract_md4c, map code_spans to CodeSpanResult.

Update Md4cScanBackend::scan_all to include code_spans.

### Step 7: Rust — Wire code spans in from_scan
File: markymark-index/src/document/mod.rs, fn from_scan

After scan_blocks (line 571), add:
\`\`\`rust
let scan_code_spans = backend.scan_code_spans(text).unwrap_or_default();
\`\`\`

In the self_cell closure, after blocks section:
- Iterate scan_code_spans, compute positions via byte_offset_to_position
- Build CodeSpanEntry with arena-allocated text
- Set code_spans field in DocumentDependent

### Step 8: Tests

**Zig tests** (zig/src/engine/document.zig):
- test_parseAll_extracts_code_spans: verify extraction.code_spans populated for "\`hello\` world"
- test_code_span_positions: verify start/end line:col correct for code span
- test_no_code_spans: verify empty for plain text
- test_code_spans_in_heading: verify "\`code\` in # Heading" produces both heading and code span

**Zig blob tests** (zig/src/engine/blob.zig):
- test_blob_code_span_struct_size: comptime assert @sizeOf(BlobCodeSpan) == 32
- test_compute_blob_size_with_code_spans: verify blob size includes code span section
- test_section_offsets_with_code_spans: verify code_spans offset is between block_ids and text_pool

**Rust from_blob tests** (markymark-index/src/document/from_blob.rs):
- test_from_blob_code_spans: generate blob with code spans, verify from_blob returns correct CodeSpanEntry
- test_from_blob_no_code_spans_backward_compat: verify v1 blob (code_span_count=0) still works
- test_from_blob_code_span_parity_with_from_scan: parse same markdown via both paths, assert code_spans match

**Rust ScanBackend tests** (markymark-core/src/scanner.rs):
- test_md4c_scan_code_spans: verify scan_code_spans returns results for backtick text
- test_scan_all_includes_code_spans: verify scan_all result includes code_spans
- test_default_scan_code_spans_empty: verify default trait impl returns empty vec

**Rust from_scan tests** (markymark-index/src/document/mod.rs or tests/):
- test_from_scan_code_spans: verify from_scan produces DocumentIndex with code_spans
- test_from_scan_code_span_accessor: verify .code_spans() accessor works

## Success Criteria
- [ ] DocumentEngine.code_spans field populated by parseAll (Zig test)
- [ ] BlobCodeSpan serialized/deserialized correctly in blob (Zig + Rust roundtrip test)
- [ ] Blob backward compatibility: v1 blobs (code_span_count=0) still parse correctly
- [ ] ScanBackend::scan_code_spans() returns code spans for backtick text
- [ ] ScanBackend::scan_all() includes code_spans field
- [ ] DocumentIndex::code_spans() accessor returns correct data via from_blob path
- [ ] DocumentIndex::code_spans() accessor returns correct data via from_scan path
- [ ] from_ast path returns empty code_spans (not wired yet, no panic)
- [ ] Parity test: from_blob and from_scan produce matching code spans for same input
- [ ] All existing tests pass: cargo nextest (regression)
- [ ] Zig tests pass: zig build test
- [ ] Zero clippy warnings: cargo clippy --workspace --all-targets
- [ ] Pre-commit hooks pass (7/7)

## Anti-Patterns (FORBIDDEN)
- NO bumping BLOB_VERSION to 2 (use _reserved bytes; save v2 for Phase B when more types arrive)
- NO unwrap/expect on blob parsing (use Result, handle gracefully)
- NO panicking on malformed blob data (return BlobError)
- NO touching RealmIndex (that's Phase A-3, after n7wx Layer 1)
- NO touching LSP/MCP surfaces (that's Phase A-3)
- NO adding code span extraction to from_ast/extract.rs (that's Phase B)
- NO sharing scan cursors between extraction types in Zig (marky-0rl6 lesson)

## Key Considerations (SRE Review)

**Edge Case: Empty code spans**
Backtick pairs with no content (\`\`\`\`\`) produce ExtractedCodeSpan with empty text. Must handle: empty string in text pool is valid, CodeSpanEntry.text = "" is valid.

**Edge Case: Extremely long code spans**
A backtick-delimited code span could span many lines in theory (CommonMark allows it). text length is u32-bounded. Existing text_pool_size overflow guard (line 505) covers this.

**Edge Case: Code spans inside headings**
Per marky-pdyo design: when in_code_span and in_heading are both true, text() appends to BOTH buffers. A heading like "# Title \`code\`" produces both a heading entry (text="Title code") and a code span entry (text="code"). Both must appear in the blob.

**Edge Case: Blob alignment**
BlobCodeSpan is extern struct. Ensure @sizeOf matches Rust's expected size. Add comptime assert in Zig and const assert in Rust. Existing pattern: BlobHeading (40 bytes), BlobLink (40 bytes). BlobCodeSpan should be 32 bytes (7 u32 fields = 28 + 4 padding).

**Ownership transfer in parseAll**
Code spans follow the same ownership pattern as headings/links: extraction.code_spans text is transferred to stored_code_spans_list, then extraction.code_spans slice container freed. texts_transferred flag applies. Scoped errdefer after toOwnedSlice protects against late OOM (marky-9m7o/marky-8nzt pattern).

**Backward compatibility is zero-cost**
v1 blobs have _reserved[0..3] as 0x00, which reads as code_span_count=0. compute_offsets produces code_spans section of size 0. No conditional version checks needed — the count field naturally handles it.

**Reference: Existing patterns to follow**
- StoredHeading → StoredCodeSpan (document.zig:34-40)
- BlobHeading → BlobCodeSpan (blob.zig:40-51)
- serializeState heading loop → code span loop (document.zig:540-570)
- from_blob HeadingData → CodeSpanData (from_blob.rs:320-327)
- from_scan heading wiring → code span wiring (mod.rs:580-602)

## Files Modified
- zig/src/engine/document.zig (StoredCodeSpan, parseAll, serializeState, freeState, freeCodeSpans)
- zig/src/engine/blob.zig (BlobCodeSpan, ScanBlobHeader.code_span_count, computeBlobSize, SectionOffsets)
- markymark-index/src/document/from_blob.rs (BlobHeader, validate_blob, compute_offsets, code span extraction)
- markymark-index/src/document/mod.rs (DocumentDependent.code_spans, code_spans() accessor, from_scan wiring, from_ast_with_overrides_opt)
- markymark-core/src/scanner.rs (CodeSpanResult, ScanBackend::scan_code_spans, ScanAllResult.code_spans, Md4cScanBackend impl)

## Files NOT Modified (out of scope)
- markymark-lsp/ (Phase A-3: surface via LSP)
- markymark-mcp/ (Phase A-3: surface via MCP)
- markymark-index/src/realm/ (Phase A-3: cross-doc index, after n7wx Layer 1)
- markymark-index/src/document/helpers.rs (no changes needed)
- zig/src/md4c/extraction_renderer.zig (already done in A-1)
- zig/src/md4c/exports.zig (FFI exports already done in A-1)
