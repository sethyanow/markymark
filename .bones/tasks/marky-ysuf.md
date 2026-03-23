---
id: marky-ysuf
title: 'autolinks.zig: fix O(n²) paren trimming in postProcessAutolinkEnd'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal
Fix O(n²) worst-case in `postProcessAutolinkEnd` by computing open/close paren
counts once after entity trimming, then decrementing incrementally during the
trim loop. GFM autolinks with many trailing unbalanced `)` currently trigger
O(n²) rescanning.

## Context
`zig/src/md4c/autolinks.zig` — `postProcessAutolinkEnd` (line 269).
This function post-processes GFM permissive autolinks (e.g., bare URLs in text
like `See https://en.wikipedia.org/wiki/Perl_(programming_language) here`).
It trims trailing unbalanced `)` so the URL ends at the correct boundary.

**Current code (lines 297-309) — O(n²):**
```zig
while (end > beg and content[end - 1] == ')') {
    var open: i32 = 0;
    var close: i32 = 0;
    for (content[beg..end]) |ch| {     // ← rescans full URL each iteration
        if (ch == '(') open += 1;
        if (ch == ')') close += 1;
    }
    if (close > open) {
        end -= 1;
    } else {
        break;
    }
}
```
Worst case: URL with N unbalanced `)` → N × N character scans = O(n²).
For a 1000-char URL ending in 100 unbalanced `)`, this is 100 × 1000 = 100,000
character comparisons instead of 1,000.

## Implementation

**File:** `zig/src/md4c/autolinks.zig`, function `postProcessAutolinkEnd`

**Replace lines 295-309** (the `// Trim trailing unbalanced `)`` block) with:

```zig
// Trim trailing unbalanced `)`: count parens once, decrement as we trim.
// O(n) total instead of O(n²) with per-iteration rescanning.
var open: i32 = 0;
var close: i32 = 0;
for (content[beg..end]) |ch| {
    if (ch == '(') open += 1;
    if (ch == ')') close += 1;
}
while (end > beg and content[end - 1] == ')' and close > open) {
    close -= 1;
    end -= 1;
}
```

**Why this is semantically identical:**
1. We count over `content[beg..end]` exactly once, after entity trimming may
   have adjusted `end`.
2. Each loop iteration removes one trailing `)`: `close -= 1`, `end -= 1`.
3. We never remove a `(`, so `open` is stable throughout.
4. The loop exits when `close <= open` (balanced) or no trailing `)` remains.
5. Removing a trailing `)` decrements `close` by exactly 1 — same as if we
   had rescanned (the removed char was counted once in the original `close`).

**Corner case: entity trimming followed by parens**
Entity trimming (lines 278-293) runs FIRST and may shorten `end`. The paren
scan happens after, over the trimmed `content[beg..end]`. Correct — we count
only the characters that remain after entity trimming.

## Regression Test to Add

Add after the existing autolink tests in `zig/src/md4c/autolinks.zig` or in
the existing test file:

```zig
test "postProcessAutolinkEnd: Wikipedia-style URL with balanced parens" {
    // GFM: https://en.wikipedia.org/wiki/Perl_(programming_language) should
    // NOT have the final ) trimmed because it is balanced.
    const content = "See https://en.wikipedia.org/wiki/Perl_(programming_language) here";
    // beg=4 (start of 'h'), end=62 (end of final ')')
    const beg: usize = 4;
    const end_in: usize = 62;
    const result = postProcessAutolinkEnd(content, beg, end_in);
    try std.testing.expectEqual(end_in, result); // no trimming — balanced
}

test "postProcessAutolinkEnd: URL with trailing unbalanced parens" {
    // URL with two extra ) at the end: example.com/foo)) — should trim both
    const content = "See https://example.com/foo)) here";
    const beg: usize = 4;
    const end_in: usize = 29; // points past second ')'
    const result = postProcessAutolinkEnd(content, beg, end_in);
    try std.testing.expectEqual(end_in - 2, result); // trims both ))
}

test "postProcessAutolinkEnd: URL with one unbalanced paren" {
    // https://example.com/foo(bar)) — one open, two close → trim one
    const content = "See https://example.com/foo(bar)) end";
    const beg: usize = 4;
    const end_in: usize = 33; // past second ')'
    const result = postProcessAutolinkEnd(content, beg, end_in);
    try std.testing.expectEqual(end_in - 1, result); // trims one )
}
```

Note: adjust `beg`/`end_in` byte positions for the actual string literals.
Use `std.mem.indexOf` or hardcode after counting carefully.

## Effort Estimate
~2 hours (1h code + 30min test + 30min verify)

## Success Criteria
- [ ] `zig build test` passes — all existing autolink tests pass
- [ ] New Wikipedia-style URL test passes (balanced parens NOT trimmed)
- [ ] New trailing unbalanced paren tests pass
- [ ] `postProcessAutolinkEnd` function has identical output as before for
  all inputs — verifiable by running existing GFM test suite
- [ ] No allocations introduced — fix is purely arithmetic/loop restructuring

## Key Considerations

**The entity-trim → paren-trim ordering is critical.** Count parens AFTER
entity trimming finishes (after line 293), not before. Entity trimming changes
`end`; if you count before, you include chars that entity trimming removes.

**`open` never changes during the while loop.** Only `close` decrements.
This is correct because we only remove `)` characters (never `(`), so
opening paren count is stable.

**The precondition `end >= beg + 3` is already enforced by `std.debug.assert`.**
This guarantees at least 3 characters in the URL (scheme minimum). The paren
count loop over `content[beg..end]` is safe — `beg < end` always holds.

**No overflow risk:** `open` and `close` are `i32`. Maximum URL length in
practice is well under 2,147,483,647. Even a 1MB URL (absurd) has at most 500K
parens, far below `i32` max.

**Test byte offset calculation:** When writing tests, count bytes carefully.
Use `const url = "https://...";` then `const beg = 4;` (after "See "),
`const end_in = 4 + url.len;` to avoid manual miscounting.

## Anti-patterns
- ❌ Do NOT recount inside the while loop — defeats the entire purpose
- ❌ Do NOT use `usize` for `open`/`close` — subtraction underflow risk if
  using unsigned (stick with `i32`)
- ❌ Do NOT add memory allocation — pure arithmetic fix only
- ❌ Do NOT change the entity-trimming logic (lines 278-293) — only the
  paren-trimming block changes
