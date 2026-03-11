---
id: marky-h6gg
title: 'CodeRabbit hardening: overflow guards, bounds checks, vendored autolink fix'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

Low-priority hardening items from CodeRabbit review. Refine and decide which to act on.

## Items

1. **blob.zig computeSectionOffsets overflow** (L130-146) — Uses u32 arithmetic that could overflow with malicious counts. Callers always call validateBlob() first (which uses u64), but the precondition isn't documented. Fix: add doc comment asserting validateBlob must be called first, or switch to u64 arithmetic.

2. **blob.zig readStruct/writeStruct bounds** (L199-210) — No bounds check before memcpy. Internal helpers called with validated offsets, but could add checked variants (readStructChecked/writeStructChecked) for defense in depth.

3. **autolinks.zig postProcessAutolinkEnd underflow** (L263-269) — j = end - 2 can theoretically underflow when end == beg + 1 and beg == 0. Vendored Bun code. Unreachable in practice (autolink paths ensure beg >= 4). Fix: add guard `end > beg + 2` before the subtraction.

4. **brza_kernels.rs parity guard masks failures** (L425-437) — Both tree-sitter and md4c can silently return 0, making assert_eq pass vacuously. Benchmark code only. Fix: use .expect() instead of .unwrap_or(0) for md4c, and propagate error from tree-sitter.

## Design

## Goal
Harden low-risk Zig and Rust code paths identified by CodeRabbit review. All items are defense-in-depth — no known exploits, but strengthen invariants for future maintainability.

## Effort Estimate
~2 hours total (4 items, ~30 min each)

## Item Analysis (SRE-reviewed)

### 1. blob.zig computeSectionOffsets — doc comment + debug assert
**File:** zig/src/engine/blob.zig L130-146
**Risk:** LOW — all 3 callers (document.zig:502, :781, :972) operate on headers from computeBlobSize or getBlob, which validate in u64 first. But function is pub and precondition undocumented.
**Fix:** Add doc comment stating precondition + debug assert at function entry.

\`\`\`zig
/// Compute offset of each section within the blob.
///
/// PRECONDITION: header must come from a validated blob (via validateBlob or
/// computeBlobSize). Section counts that overflow u32 arithmetic are undefined
/// behavior. Use computeBlobSize() first to validate.
pub fn computeSectionOffsets(header: ScanBlobHeader) SectionOffsets {
    // Debug-mode check: verify total fits in u32 (catches misuse in tests)
    if (std.debug.runtime_safety) {
        std.debug.assert(computeBlobSize(
            header.heading_count, header.link_count, header.tag_count,
            header.block_id_count, header.line_count, header.text_pool_size,
        ) != null);
    }
    // ... rest unchanged
\`\`\`

### 2. blob.zig readStruct/writeStruct — debug bounds assert
**File:** zig/src/engine/blob.zig L199-210
**Risk:** LOW — all callers use offsets from computeSectionOffsets on validated blobs. But pub functions with no bounds check.
**Fix:** Add debug assert before memcpy. No API change (keeps void/T return types).

\`\`\`zig
pub fn writeStruct(comptime T: type, buf: []u8, offset: usize, value: T) void {
    std.debug.assert(offset + @sizeOf(T) <= buf.len);  // bounds check (debug only)
    const src: [*]const u8 = @ptrCast(&value);
    @memcpy(buf[offset..][0..@sizeOf(T)], src[0..@sizeOf(T)]);
}

pub fn readStruct(comptime T: type, buf: []const u8, offset: usize) T {
    std.debug.assert(offset + @sizeOf(T) <= buf.len);  // bounds check (debug only)
    var result: T = undefined;
    const dst: [*]u8 = @ptrCast(&result);
    @memcpy(dst[0..@sizeOf(T)], buf[offset..][0..@sizeOf(T)]);
    return result;
}
\`\`\`

### 3. autolinks.zig postProcessAutolinkEnd — doc comment only (NO code change)
**File:** zig/src/md4c/autolinks.zig L255-269
**Risk:** NONE — SRE analysis confirms underflow is unreachable:
  - URL path: end >= beg + 6 (scheme len 3+ plus "//")
  - WWW path: end >= beg + 4 (pos+1 where pos >= beg+3)
  - Email path: does NOT call postProcessAutolinkEnd
**Fix:** Add doc comment explaining the invariant. Do NOT change vendored logic.

\`\`\`zig
/// GFM post-processing: trim trailing unbalanced ')' and entity-like suffixes.
///
/// INVARIANT: end >= beg + 3 for all callers (URL: scheme >= 3 + "//",
/// WWW: "www." prefix). The j = end - 2 subtraction is safe.
fn postProcessAutolinkEnd(content: []const u8, beg: usize, end_in: usize) usize {
\`\`\`

### 4. brza_kernels.rs parity guard — use expect() for fail-fast
**File:** markymark-kernels/benches/brza_kernels.rs L425-437
**Risk:** LOW — benchmark code, not production. But vacuous 0==0 is bad practice.
**Fix:** Replace .unwrap_or(0) and silent eprintln fallback with explicit panics.

\`\`\`rust
let ts_count = match parser.parse(&check_doc) {
    Ok(ast) => ast.root_elements().iter().filter(|e| e.as_heading().is_some()).count(),
    Err(err) => panic!("tree-sitter parity check failed: {err}"),
};
let md4c_count = md4c_backend
    .scan_headings(&check_doc)
    .expect("md4c parity check failed")
    .len();
\`\`\`

Note: count_tree_sitter_headings is also used in the hot benchmark loop where 0-on-error is fine (don't panic mid-benchmark). Only the one-time parity guard needs fail-fast.

## Success Criteria
- [ ] computeSectionOffsets has doc comment with precondition + debug assert
- [ ] readStruct/writeStruct have debug bounds asserts
- [ ] postProcessAutolinkEnd has invariant doc comment (no code change)
- [ ] Benchmark parity guard panics on failure instead of defaulting to 0
- [ ] All existing tests pass: \`zig build test\` and \`cargo nextest\`
- [ ] No new warnings from clippy or zig build

## Anti-patterns
- Do NOT change autolinks.zig control flow (vendored code)
- Do NOT change readStruct/writeStruct return types (callers expect non-error)
- Do NOT add runtime checks in release builds for items 1-2 (debug asserts only)
- Do NOT change count_tree_sitter_headings behavior in benchmark loop (only parity guard)

## Test Plan
No new tests needed — these are doc comments and debug asserts on already-tested paths. Existing test suites validate the paths. The debug asserts will fire in test builds (which run in debug mode) if invariants are ever violated.
