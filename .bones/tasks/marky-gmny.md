---
id: marky-gmny
title: 'Bug: extractFromMarkdown double-free when ext.oom=true (errdefer + explicit deinit)'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

Discovered while writing OOM injection tests for marky-9m7o.

extractFromMarkdown has errdefer ext.deinit() at line 496. But three
explicit ext.deinit() calls also exist:
- line 510: when ext.oom == true → returns OOM → errdefer fires again
- line 516: headings.toOwnedSlice fail → returns OOM → errdefer fires again
- line 521: links.toOwnedSlice fail → returns OOM → errdefer fires again

All three trigger a double-free. In GPA debug mode, this fills freed memory
with 0xaa and the second deinit crashes with segfault at 0xaaaaaaaaaaaaaaaa.

This only surfaces under OOM injection (FailingAllocator). The existing T1-4
test uses fail_index=0 (fails before any callbacks run), so oom=false and
only the errdefer path fires (no double-free). Loop-based OOM tests would hit
fail_index values where oom=true.

Fix: Remove explicit ext.deinit() calls, rely solely on errdefer. For the
success path (lines 526-531), the explicit deinit of accumulation buffers and
empty lists is correct (errdefer won't fire on success). The error paths
should not call ext.deinit() explicitly — just return the error.

Note: For the links.toOwnedSlice failure path, headings (already transferred
via toOwnedSlice at line 515) would be leaked if we just drop the explicit
deinit. Need to add a flag or separate cleanup for partially-transferred data.

## Design

## Bug: extractFromMarkdown double-free + heading text leak on OOM error paths

### Root Cause Analysis

extractFromMarkdown (extraction_renderer.zig:490) has errdefer ext.deinit() at line 496.
Three explicit ext.deinit() calls on error paths cause double-free when errdefer also fires.

Additionally, the links.toOwnedSlice failure path has a SEPARATE heading text string leak:
allocator.free(headings) at line 520 frees the slice backing array but NOT the .text
strings inside each ExtractedHeading. After toOwnedSlice, ext.headings is empty, so
ext.deinit() won't iterate those heading texts either.

### Three Error Paths Affected

**Path 1 — ext.oom == true (line 509-512):**
- ext.deinit() at 510 frees everything
- return error.OutOfMemory triggers errdefer ext.deinit() → double-free

**Path 2 — headings.toOwnedSlice fails (line 515-518):**
- ext.deinit() at 516 frees everything (ext.headings still owns items)
- return error.OutOfMemory triggers errdefer → double-free

**Path 3 — links.toOwnedSlice fails (line 519-523):**
- headings already transferred out of ext.headings via toOwnedSlice (ext.headings is empty)
- allocator.free(headings) at 520 frees slice array but NOT heading .text strings → leak
- ext.deinit() at 521 frees ext (but ext.headings.items is empty, so heading texts leaked)
- return error.OutOfMemory triggers errdefer → double-free

### Fix Strategy

Remove ALL explicit ext.deinit() calls on error paths. Rely solely on errdefer.
Add a scoped errdefer after headings.toOwnedSlice to clean up transferred headings:

\`\`\`zig
const headings = ext.headings.toOwnedSlice(allocator) catch {
    return error.OutOfMemory;  // errdefer ext.deinit() handles cleanup
};
errdefer {
    for (headings) |h| allocator.free(h.text);
    allocator.free(headings);
}

const links = ext.links.toOwnedSlice(allocator) catch {
    return error.OutOfMemory;  // headings errdefer + ext errdefer handle cleanup
};
\`\`\`

Success path (lines 525-531) stays unchanged — manual buffer/list cleanup is correct
since errdefer doesn't fire on success.

### Implementation Checklist

- [ ] Step 1: Write failing OOM-loop test using FailingAllocator + GPA
      File: zig/src/md4c/extraction_renderer.zig (test section at bottom)
      Test name: "extractFromMarkdown OOM loop: no double-free or leak"
      Iterate fail_index from 0..N, verify OutOfMemory or success, never crash.
      GPA detects double-free (fills freed memory with 0xaa → segfault on second free).
- [ ] Step 2: Run test, confirm it crashes/fails (RED) on current code
- [ ] Step 3: Remove explicit ext.deinit() from all 3 error paths (lines 510, 516, 521)
- [ ] Step 4: Remove allocator.free(headings) at line 520 (incomplete cleanup, now handled by errdefer)
- [ ] Step 5: Add errdefer block after headings.toOwnedSlice for transferred heading cleanup
- [ ] Step 6: Run test, confirm it passes (GREEN)
- [ ] Step 7: Run full test suite: zig build test in zig/ directory
- [ ] Step 8: Run Rust tests: cargo nextest
- [ ] Step 9: Commit

### Success Criteria

- [ ] New OOM-loop test passes (iterates 20+ fail_indices without crash)
- [ ] GPA reports no memory leaks at end of each iteration (leak check via gpa.deinit())
- [ ] All existing extraction_renderer tests still pass
- [ ] Full zig build test passes
- [ ] cargo nextest passes (Rust integration unaffected)
- [ ] No double-free detected under any fail_index value

### Anti-patterns

- DO NOT use a boolean flag to track partial transfer state — use errdefer scoping
- DO NOT suppress GPA leak detection in tests — leaks must be caught
- DO NOT modify ExtractionRenderer.deinit() — it's correct for its intended use
- DO NOT change the success path cleanup (lines 525-531) — it's correct

### Edge Cases

**toOwnedSlice semantics:** After successful toOwnedSlice, the ArrayList is reset
(items.len=0, capacity=0). ext.deinit() on the empty ArrayList is a no-op for items
but still calls .deinit(allocator) which is safe on an empty list.

**GPA behavior on double-free:** In debug/safe mode, GPA fills freed memory with 0xaa.
Second free attempts to read metadata at 0xaaaaaaaaaaaaaaaa → guaranteed segfault.
This is how the test detects the bug.

**FailingAllocator fail_index=0:** Already tested by T1-4. Fails before any callbacks
run, so oom flag is false and only errdefer path fires. This is the ONLY index that
doesn't trigger the bug (no callback allocations means no oom=true).

**FailingAllocator at callback-allocation indices:** These set ext.oom=true, triggering
Path 1. The exact fail_indices depend on document complexity.

**FailingAllocator at toOwnedSlice indices:** These trigger Paths 2 or 3 depending
on which toOwnedSlice fails.
