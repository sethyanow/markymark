---
id: marky-0mr.6
title: 'PR#39 review: fix inlines.zig error handling'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---



Fix three issues in zig/src/md4c/inlines.zig:

**T2-1: Memory leak — defer free(resolved) may not cover all error paths (line ~68)**
resolved is dupe()'d then defer free()'d — but if the code exits before reaching the defer due to an earlier error, the allocation leaks. Fix: restructure so errdefer self.allocator.free(resolved) fires on any error exit after successful allocation.

**T2-12: Silent catch {} in emphasis delimiter collection (line ~476-484)**
If emph_delims.append() fails, delimiters are silently dropped, causing emphasis markers to render as literal text. Fix: propagate via try, or set an oom flag on the parser state and handle deterministically.

**T3-12: Wrapping subtraction closer_idx -%= 1 needs comment (line ~569-576)**
The pattern decrements so the while loop's post-increment brings back to the same index for re-processing. Non-obvious control flow trick needs an explanatory comment.

Source: PR #39 review — Copilot + CodeRabbit
