---
id: marky-pdyo
title: 'Phase A-1: CodeSpanEntry type + Zig ExtractionRenderer code span capture'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---



## Design

## Goal
Add foundational code span types and Zig extraction. Does NOT depend on fgl8 (no extract.rs changes).

## Effort Estimate
6-8 hours across 4 files (Zig extraction_renderer, Zig md4c/exports, Rust types, Rust FFI bindings).

## Implementation

### 1. CodeSpanEntry type (Rust)
File: markymark-index/src/document/types.rs

Add CodeSpanEntry<'arena> following WikiLinkEntry pattern (types.rs:67-99):
```rust
#[derive(Debug, Clone)]
pub struct CodeSpanEntry<'arena> {
    pub text: &'arena str,       // backtick content (decoded)
    pub range: Range,
    pub start_byte: usize,       // byte offset of opening backtick
    pub end_byte: usize,         // byte offset past closing backtick
    pub language_hint: Option<&'arena str>,  // None for Tier 1
    pub kind: Option<SymbolKind>,            // None for Tier 1
}
```

Add CodeSpanOwned (same fields, String instead of &str).
Add code_spans: Option<Vec<CodeSpanOwned>> to IncrementalOverrides (types.rs:157-176).

### 2. ExtractedCodeSpan (Zig)
File: zig/src/md4c/extraction_renderer.zig

Add struct after ExtractedLink (line 33):
```zig
pub const ExtractedCodeSpan = struct {
    text: []const u8,   // owned decoded code span text (allocator-backed)
    offset: u32,        // byte offset of opening backtick in source
    end_offset: u32,    // byte offset past closing backtick in source
};
```

Add to ExtractionResult (line 35):
- code_spans: std.ArrayListUnmanaged(ExtractedCodeSpan) field
- deinit: free code_spans text strings + list

Add to ExtractionRenderer struct (after line 80):
- code_spans: std.ArrayListUnmanaged(ExtractedCodeSpan)
- code_scan_cursor: u32 = 0 (SEPARATE cursor per marky-0rl6 lesson)
- in_code_span: bool = false
- code_text_buf: std.ArrayListUnmanaged(u8)

### 3. ExtractionRenderer code span accumulation
File: zig/src/md4c/extraction_renderer.zig

Pattern follows heading/link accumulation exactly:

enterSpan(.code):
- Set in_code_span = true
- clearRetainingCapacity() on code_text_buf

text() callback (line 231-247):
- Add: if (self.in_code_span) append effective text to code_text_buf
- NOTE: in_code_block early return (line 232) fires BEFORE in_code_span check.
  This is correct: md4c does NOT fire SpanType.code inside fenced code blocks.

leaveSpan(.code):
- Set in_code_span = false
- Call finalizeCodeSpan()

finalizeCodeSpan() — new function following finalizeLink pattern (line 268):
- toOwnedSlice(code_text_buf) for text
- findCodeSpanOffset() for opening backtick byte offset
- Compute end_offset from offset + backtick delimiters + text length
- Append ExtractedCodeSpan to code_spans list
- On OOM: set self.oom = true, return

findCodeSpanOffset() — new function following findLinkOffset pattern (line 397):
- Scan forward from code_scan_cursor through src_text for backtick character
- Advance code_scan_cursor past the matched span
- Return byte offset of opening backtick

deinit() — extend (line 92):
- Free code_text_buf
- Free code_spans text strings + list

### 4. CMd4cResult extension (Zig FFI)
File: zig/src/md4c/exports.zig (NOT engine/exports.zig)

Add CMd4cCodeSpan extern struct:
```zig
pub const CMd4cCodeSpan = extern struct {
    source_offset: u32,   // byte offset of opening backtick
    end_offset: u32,      // byte offset past closing backtick
    text_offset: u32,     // offset into text_blob
    text_length: u32,     // length in text_blob
};
// comptime assert @sizeOf == 16
```

Extend CMd4cResult:
- Add code_spans: ?[*]CMd4cCodeSpan pointer field
- Add code_spans_count: u32 count field
- Update _padding and comptime size assert

Extend marky_md4c_extract (line 65):
- Include code span text in blob_size calculation
- Allocate c_code_spans array
- Pack code span text into text_blob
- Fill CMd4cCodeSpan structs with blob offsets
- Free on error paths (errdefer cascade pattern from lines 120-138)

Extend marky_md4c_free (line 212):
- Free code_spans array if non-null

### 5. Rust FFI bindings update
File: markymark-kernels/src/md4c.rs (or wherever CMd4cResult is mirrored in Rust)

Update #[repr(C)] Rust mirror struct to match new CMd4cResult layout:
- Add code_spans pointer field
- Add code_spans_count field
- Add CMd4cCodeSpan repr(C) struct

## Success Criteria
- [ ] Zig ExtractionRenderer extracts inline code spans with verified byte offsets (specific assertions in tests below)
- [ ] Zig tests: `zig build test` passes with >= 8 new code span test cases
- [ ] CMd4cResult extended with code_spans; FFI tests verify round-trip through marky_md4c_extract
- [ ] Rust CodeSpanEntry<'arena> and CodeSpanOwned types defined with correct fields
- [ ] Rust FFI binding struct matches Zig CMd4cResult layout (verified by existing ABI test or new one)
- [ ] Code spans inside fenced code blocks are NOT extracted (negative test)
- [ ] Code spans interleaved with headings/links produce correct offsets for ALL types (regression)
- [ ] OOM during code span accumulation sets oom flag, does not panic or leak
- [ ] cargo nextest passes (all existing tests unaffected)
- [ ] cargo clippy clean
- [ ] Pre-commit hooks pass

## Tests (Specific)

### Zig extraction_renderer tests:
- test_code_span_basic: `here is \`hello\` world` -> 1 code span, text="hello", offset at backtick position
- test_code_span_double_backtick: `\`\`code with \`backtick\`\`` -> text="code with \`backtick\`"
- test_code_span_in_heading: `# Title \`code\`` -> 1 heading + 1 code span, both with correct offsets
- test_code_span_in_link: `[\`code\`](url)` -> 1 link + 1 code span
- test_code_span_in_fenced_block_not_extracted: fenced block content yields 0 code spans
- test_code_span_empty_backticks: `\`\`` -> 1 code span, text="" (empty string)
- test_code_span_multiple: `\`a\` then \`b\`` -> 2 code spans in order with ascending offsets
- test_code_span_entity_decoded: `\`a &amp; b\`` -> text="a & b" (entity decoded)

### Zig md4c/exports tests:
- test_md4c_extract_code_span: verify CMd4cResult has correct code_spans_count and text via text_blob
- test_md4c_extract_mixed_doc: heading + link + code span all extracted, all blob offsets valid
- test_md4c_extract_no_code_spans: document without backticks has code_spans_count=0

### Rust type tests:
- test_code_span_owned_to_entry: verify arena allocation round-trip (text, range, byte offsets preserved)
- test_incremental_overrides_code_spans: verify Option<Vec<CodeSpanOwned>> field exists and works

## Anti-Patterns (FORBIDDEN)
- NO shared scan cursor between code spans and headings/links (marky-0rl6: shared cursor caused offset corruption)
- NO silent catch {} for OOM in code span accumulation (use self.oom flag pattern like heading/link, line 242)
- NO extracting code spans from inside fenced code blocks (in_code_block early return must fire before in_code_span check)
- NO unwrap/expect in Rust production code
- NO TODOs without issue numbers
- NO changing CMd4cResult size without updating all comptime asserts
- NO adding code span extraction to extract.rs in this task (that's Phase A-4, after fgl8)

## Key Considerations (SRE Review)

### Edge Case: Code span nested inside heading
When `# Title \`code\` rest` is parsed, md4c fires: enterBlock(heading) -> text("Title ") -> enterSpan(code) -> text("code") -> leaveSpan(code) -> text(" rest") -> leaveBlock(heading).
Both in_heading and in_code_span are true simultaneously. text() must append to BOTH heading_text_buf AND code_text_buf when both flags are set. The heading text will include the code span text ("Title code rest"), which is correct (headings include inline content).

### Edge Case: Code span in link text
`[\`code\`](url)` — in_link and in_code_span both true. text() appends to both link_text_buf and code_text_buf. Code span is extracted independently from the link.

### Edge Case: Empty backticks
`\`\`` produces enterSpan(code) immediately followed by leaveSpan(code) with no text() callback between them. finalizeCodeSpan must handle empty code_text_buf (toOwnedSlice returns empty slice, which is valid).

### Edge Case: Double backtick delimiters
md4c handles `\`\`code with \`backtick\`\`` — enterSpan/leaveSpan fire once. The delimiter length affects offset calculation: findCodeSpanOffset must scan for the actual backtick run (could be 1, 2, or 3 backticks).

### Edge Case: OOM during accumulation
If code_text_buf.appendSlice or code_spans.append fails, set self.oom = true and return. Follow existing pattern (lines 242, 245). Do NOT panic. Caller (extractFromMarkdown, line 491+) checks oom flag.

### Edge Case: end_offset calculation
end_offset = position past the closing backtick delimiter. Since md4c doesn't give us the closing delimiter position directly, findCodeSpanOffset must advance code_scan_cursor past the entire `...` span (opening backticks + content + closing backticks). Return end_offset from cursor position after finding the closing backticks.

### ABI Compatibility
CMd4cResult layout change is a BREAKING ABI change. Both Zig struct and Rust #[repr(C)] mirror must be updated in the same commit. Verify with comptime size assert on Zig side and #[cfg(test)] size assertion on Rust side.

## Codebase Entry Points
- WikiLinkEntry pattern: markymark-index/src/document/types.rs:67-99
- IncrementalOverrides: markymark-index/src/document/types.rs:157-176
- ExtractionRenderer fields: zig/src/md4c/extraction_renderer.zig:55-83
- enterSpan: zig/src/md4c/extraction_renderer.zig:181-206
- leaveSpan: zig/src/md4c/extraction_renderer.zig:208-227
- text callback: zig/src/md4c/extraction_renderer.zig:231-247
- finalizeLink pattern: zig/src/md4c/extraction_renderer.zig:268-295
- findLinkOffset pattern: zig/src/md4c/extraction_renderer.zig:397-489
- deinit: zig/src/md4c/extraction_renderer.zig:92-108
- CMd4cResult: zig/src/md4c/exports.zig:44-55
- marky_md4c_extract: zig/src/md4c/exports.zig:65-206
- marky_md4c_free: zig/src/md4c/exports.zig:212-233
- Rust FFI bindings: markymark-kernels/src/md4c.rs (find CMd4cResult repr(C) mirror)
