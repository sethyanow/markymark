---
id: marky-8nzt
title: parseAll toOwnedSlice cascade leak on partial OOM failure
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

Sequential toOwnedSlice calls at document.zig:359-362 without scoped errdefer. If headings succeeds but links fails, headings data leaks (stored_headings_list consumed by toOwnedSlice, errdefer frees empty list). Distinct from marky-9m7o texts_transferred fix. Source: codex review.

## Design

## Goal

Add scoped errdefer guards after each toOwnedSlice in parseAll to prevent memory leaks when a later toOwnedSlice fails.

## Root Cause

document.zig:359-362 has four sequential toOwnedSlice calls:
\`\`\`zig
out_headings.* = stored_headings_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
out_links.* = stored_links_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
out_tags.* = stored_tags_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
out_block_ids.* = stored_block_ids_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
\`\`\`

After headings.toOwnedSlice succeeds:
- stored_headings_list is empty (consumed by toOwnedSlice)
- out_headings.* now holds the owned slice
- If links.toOwnedSlice fails → errdefer fires → freeStoredHeadingsList runs on empty list → no-op
- Data in out_headings.* leaks (nobody frees it)

Same cascade for each subsequent call. The existing marky-9m7o texts_transferred fix handles OOM in steps 6-9 (before toOwnedSlice), not the toOwnedSlice cascade itself.

## Effort Estimate

2-3 hours (scoped errdefer pattern + OOM regression test)

## Implementation Checklist

- [ ] After line 359 (headings toOwnedSlice success), add scoped errdefer:
  \`\`\`zig
  out_headings.* = stored_headings_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
  errdefer freeHeadings(allocator, out_headings.*);
  \`\`\`
- [ ] After line 360 (links toOwnedSlice success), add scoped errdefer:
  \`\`\`zig
  out_links.* = stored_links_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
  errdefer freeLinks(allocator, out_links.*);
  \`\`\`
- [ ] After line 361 (tags toOwnedSlice success), add scoped errdefer:
  \`\`\`zig
  out_tags.* = stored_tags_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
  errdefer freeTags(allocator, out_tags.*);
  \`\`\`
- [ ] No errdefer needed after block_ids (last in sequence, nothing can fail after it except line_starts/scalar assignments which don't allocate)
- [ ] Add OOM-loop regression test using FailingAllocator pattern (from marky-gmny):
  - Name: \`test_parseAll_toOwnedSlice_cascade_no_leak\`
  - Iterate fail_index targeting the toOwnedSlice allocation range
  - Use GPA to detect leaks (returns .leak status)
  - Verify no leak detected at each fail_index
- [ ] Run \`zig build test\` — all Zig tests pass
- [ ] Run \`cargo nextest\` — all Rust tests pass (integration unchanged)

## Success Criteria

- [ ] Scoped errdefer after each of the first 3 toOwnedSlice calls
- [ ] OOM-loop test passes with GPA detecting no leaks at any fail_index
- [ ] Existing parseAll tests still pass
- [ ] \`zig build test\` clean (all 614+ Zig tests)
- [ ] \`cargo nextest\` clean (all 1002+ Rust tests)

## Key Considerations (SRE Review)

**Edge Case: All four toOwnedSlice calls fail**
If headings.toOwnedSlice fails (first call), the existing errdefer handles it correctly — stored_headings_list is still populated and gets freed. No new code needed for this case.

**Edge Case: Only last call (block_ids) fails**
Headings, links, and tags are all transferred. Scoped errdefers for all three fire, freeing all transferred data. This is the most complex case — verify in test.

**Edge Case: line_starts errdefer interaction**
line_starts has its own errdefer at line 225. When a toOwnedSlice fails, both the line_starts errdefer AND the scoped toOwnedSlice errdefers fire. These are independent allocations with no interaction — safe.

**Interaction with texts_transferred flag**
The toOwnedSlice cascade runs at line 359 AFTER texts_transferred is set to true at line 292. The top-level errdefer (lines 213-218) also fires. After toOwnedSlice empties stored_headings_list, the top-level errdefer's freeStoredHeadingsList runs on the empty list (no-op). The new scoped errdefer runs freeHeadings on out_headings.* (actual cleanup). No double-free risk because the list and the owned slice are disjoint.

**Reference: marky-gmny OOM-loop pattern**
See MEMORY.md "Zig errdefer + explicit deinit = double-free pattern" section. Use the same FailingAllocator + GPA pattern for testing.

## Anti-patterns
- Do NOT remove the top-level errdefer (lines 213-218) — it handles pre-toOwnedSlice failures
- Do NOT restructure to atomic commit — scoped errdefer is simpler and idiomatic Zig
- Do NOT use explicit deinit/free in catch blocks — use errdefer (marky-gmny lesson)
