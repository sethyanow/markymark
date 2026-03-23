---
id: marky-0rl6
title: 'PR#41 fix: ExtractionRenderer scan_cursor corrupts heading offsets on nested elements'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Fix the shared scan_cursor bug in ExtractionRenderer that produces wrong heading
byte offsets when a link is nested inside a heading.

## Root Cause

zig/src/md4c/extraction_renderer.zig:62 has a single mutable field scan_cursor
shared by findHeadingOffset() (line 295) and findLinkOffset() (line 396). md4c
fires leaveSpan(.a) before leaveBlock(.h) for nested elements, so finalizeLink()
runs before finalizeHeading(). findLinkOffset() advances scan_cursor past the link
syntax. findHeadingOffset() then starts from the advanced position, misses the
heading entirely, and returns a garbage fallback offset.

Confirmed trace for input "# See [here](url)\n":
1. finalizeLink() → findLinkOffset() starts scan_cursor=0, finds [ at byte 6, sets scan_cursor=18
2. finalizeHeading() → findHeadingOffset() starts from scan_cursor=18, scans past end, returns fallback=18
3. Heading offset stored as 18 (WRONG — should be 0)

Existing test gap: test "link inside heading" (line 697) only asserts text content, not offsets.

## Effort Estimate

2-4 hours (focused Zig change + tests)

## Success Criteria

- [ ] Heading offset for "# See [here](url)\n" is 0 (the # position), not 18
- [ ] Wiki link inside heading "# See [[target]]\n" also produces correct heading offset
- [ ] Autolink inside heading "# See <https://x.com>\n" also produces correct heading offset
- [ ] All existing extraction_renderer tests pass (no regressions on other offsets)
- [ ] OOM loop test at line 971 still passes (no double-free from new cursor field)
- [ ] New regression test asserts heading.offset AND link.offset for "# See [here](url)\n"
- [ ] zig build test passes for md4c tests

## Implementation Checklist

- [ ] In ExtractionRenderer struct (line 55-82): replace scan_cursor field with two independent
      fields: heading_scan_cursor and link_scan_cursor (both u32, default 0)
- [ ] findHeadingOffset (line 295): change self.scan_cursor reads/writes to self.heading_scan_cursor
- [ ] findLinkOffset (line 396): change self.scan_cursor reads/writes to self.link_scan_cursor
- [ ] finalizeLink (line 279): change self.scan_cursor read to self.link_scan_cursor for end_offset
- [ ] Add regression test: "link inside heading has correct offsets" — assert heading.offset==0,
      link.offset==6, link.end_offset==18
- [ ] Add test: "wiki link inside heading has correct offsets"
- [ ] Add test: "autolink inside heading has correct offsets"
- [ ] Run full test suite: zig build test (extraction_renderer + engine + md4c tests)

## Key Considerations (SRE Review)

**Edge Case: Multiple links in one heading**
"# [a](x) and [b](y)\n" — both links should get correct offsets, heading offset
should be 0. link_scan_cursor advances monotonically through both links.

**Edge Case: Consecutive headings with links**
"# [a](x)\n## [b](y)\n" — second heading's heading_scan_cursor should start past
the first heading. Both findHeadingOffset and findLinkOffset advance their own cursors
independently, so the monotonic scan property holds for both.

**Edge Case: Wiki link inside heading**
"# See [[target]]\n" — wiki links use different findLinkOffset path (line 400-417).
Must verify this path also uses link_scan_cursor.

**No impact on init/deinit:** The new fields are plain u32 scalars with no allocations.
The OOM loop test exercises allocation failure paths, not cursor state, so it is unaffected.

**Alternative considered: Save/restore scan_cursor**
Could save scan_cursor before findLinkOffset and restore after. Rejected: this breaks
the monotonic-advance invariant that both functions rely on. Split cursors preserve
monotonicity for each scan path independently.

## Anti-patterns

- Do NOT use a save/restore pattern (breaks monotonic advance)
- Do NOT add a third cursor "just in case" — heading and link are the only two scan paths
- Do NOT change the md4c callback ordering (that's the parser's responsibility)
