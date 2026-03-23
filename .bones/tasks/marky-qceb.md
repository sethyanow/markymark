---
id: marky-qceb
title: 'Zig document.zig safety hardening: OOM error mapping, text_pool overflow, errdefer guard'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Three Copilot PR #40 findings in zig/src/engine/document.zig. C5: OOM mapped to ParseFailed. C6: text_pool_size u32 overflow. C1: errdefer free of static empty slice.

## Design

## Goal
Fix three safety issues in zig/src/engine/document.zig found by Copilot on PR #40. Two are P2 correctness/safety bugs; one is a P4 code smell bundled for efficiency since all are in the same file.

## Effort Estimate
2-3 hours

## Fix 1 (P2): OOM mapped to ParseFailed (Copilot C5)

**Problem**: Line 193-194 catches ALL extractFromMarkdown errors and maps them to error.ParseFailed. The error set is CallbackError(OutOfMemory) || error{StackOverflow, InputTooLarge}. OOM reports as parse failure (-4) across FFI instead of OOM (-3).

**Fix**: Replace blanket catch with switch on error type:
\`\`\`zig
var extraction = extraction_renderer.extractFromMarkdown(text, allocator) catch |e| return switch (e) {
    error.OutOfMemory => error.OutOfMemory,
    error.StackOverflow, error.InputTooLarge => error.ParseFailed,
};
\`\`\`

**Test**: Existing tests cover happy path. OOM is hard to unit test without a failing allocator, but the fix is a straightforward error propagation change. Verify no regression with zig build test.

## Fix 2 (P2): text_pool_size u32 overflow (Copilot C6)

**Problem**: Lines 459-473 accumulate text_pool_size as u32. Input is bounded to u32 by InputTooLarge, but total pool can be ~2-4x input size (text+slug for headings, text+target for links). If pool exceeds u32::MAX, the value wraps BEFORE being passed to computeBlobSize, so the overflow check passes with a wrong (small) value, causing undersized allocation and out-of-bounds @memcpy writes.

**Fix**: Change text_pool_size from u32 to u64. After accumulation, check if > maxInt(u32) and return error.OutOfMemory if so. Cast to u32 only after the check:
\`\`\`zig
var text_pool_size: u64 = 0;
// ... accumulate ...
if (text_pool_size > std.math.maxInt(u32)) return error.OutOfMemory;
const text_pool_u32: u32 = @intCast(text_pool_size);
\`\`\`
Then pass text_pool_u32 to computeBlobSize and use it for the header.

**Test**: Add a test that creates a DocumentEngine with content designed to have large pool. However, for a true overflow test we'd need >4GB of string data, which is impractical. Instead, verify the fix compiles and existing tests pass. The fix is structurally safe — u64 arithmetic cannot wrap for realistic inputs.

## Fix 3 (P4): errdefer free of static empty slice (Copilot C1)

**Problem**: Line 216 has errdefer allocator.free(line_starts). When text is empty, computeLineStarts returns &.{} (static zero-length slice). Zig's Allocator.free is a no-op for zero-length slices, so this isn't a runtime bug, but it's a code smell that could break if the allocator changes.

**Fix**: Guard the errdefer:
\`\`\`zig
errdefer if (line_starts.len > 0) allocator.free(line_starts);
\`\`\`

**Test**: Existing test_create_destroy_no_leaks and empty document tests cover this path. No new test needed.

## Success Criteria
- [ ] extractFromMarkdown OOM propagates as error.OutOfMemory (not ParseFailed)
- [ ] text_pool_size computed in u64 with overflow check before u32 cast
- [ ] errdefer line_starts free guarded by len > 0
- [ ] zig build test passes (all engine tests)
- [ ] cargo check passes (Rust side unaffected)
- [ ] cargo nextest passes (full workspace)

## Implementation Checklist
- [ ] Fix 1: Change catch on line 193-194 to switch on error type
- [ ] Fix 2: Change text_pool_size to u64 on line 459, add overflow check after line 473, update header construction to use u32 cast
- [ ] Fix 3: Add len > 0 guard to errdefer on line 216
- [ ] Run: zig build test (engine tests)
- [ ] Run: cargo check (Rust compilation)
- [ ] Run: cargo nextest (full test suite)
- [ ] Commit with message referencing marky-qceb

## Anti-patterns
- Do NOT change the error type of DocumentEngine.Error — it already has OutOfMemory and ParseFailed
- Do NOT add new Zig error types — use the existing error set
- Do NOT change serializeState return type — it already returns ![]u8
- Do NOT use @truncate for the u32 cast — use @intCast after explicit bounds check
