---
id: marky-9m7o
title: 'Zig engine: parseAll errdefer ownership leak + link end_offset heuristic'
status: closed
type: bug
priority: 4
owner: sethyanow@users.noreply.github.com
---

Two low-impact correctness issues in parseAll (document.zig):

**1. errdefer leaks text on late-stage OOM (lines 205-211, 289-290, 630-641)**
After ownership transfer point (line 289-290), extraction containers are freed but texts
are only owned by stored_*_list. If OOM occurs in steps 6-9 (tag/block scanning or
toOwnedSlice), errdefer fires but freeStoredHeadingsList only frees slugs (not texts)
and freeStoredLinksList frees nothing. Texts leak.

Fix: Add boolean texts_transferred flag, set after line 290, update errdefer to free
texts when true. Must avoid double-free with explicit extraction.deinit() in catch blocks.

**2. Link end_offset heuristic inaccuracy (lines 257-270)**
Computed end_offset assumes source text = text_len + target_len + 4. Wrong for:
- Reference links [text][ref]: target = resolved URL, not "ref"
- Titled links [text](url "title"): title adds unaccounted length
- Autolinks <url>: different syntax

Impact: cosmetic only — LSP hover/highlight ranges off by a few chars for these types.
Navigation uses start offset (correct). Proper fix requires adding end_offset to
ExtractedLink in extraction_renderer.

Source: CodeRabbit review of PR #40
