---
id: marky-0mr.3
title: 'PR#39 review: fix extraction renderer correctness'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Fix three issues in zig/src/md4c/extraction_renderer.zig:

**T1-4: Silent OOM causes truncated output while extractFromMarkdown returns success**
Multiple catch {} / catch return paths on buffer growth and toOwnedSlice mean OOM yields partial headings/links with a success return — silent data loss. Fix: add oom: bool = false field to renderer; set it on any allocation failure; check after rendering and return error.OutOfMemory. Applies to lines 76, 176-241, 246-276, 429-468.

**T1-5: findHeadingOffset/findLinkOffset misattribute offsets from code blocks or mid-line markers**
Scan for # / [ doesn't enforce line-start or skip fenced code blocks. A # or [ inside a code fence before the real heading/link corrupts byte offset ranges in downstream indices. Fix: constrain scan to line-start positions only (allowing up to 3 leading spaces + > blockquote prefix); track in_fence state when scanning; only advance scan_cursor on valid non-fenced matches. Needs a regression test with markdown containing # inside a code fence.

**T2-5: Comment contradicts code — entities ARE decoded**
Comment at line 63 says entity references are NOT decoded, but text() at lines 226-242 explicitly calls helpers.decodeEntityToUtf8. Fix: update comment to match reality.

Source: PR #39 review — CodeRabbit (Major) + Copilot
