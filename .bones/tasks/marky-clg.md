---
id: marky-clg
title: Fix uninitialized written parameter in Zig scan FFI functions
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

CodeRabbit review found that when cap==0, marky_scan_headings/links/tags/block_ids return -2 but leave the written pointer uninitialized, causing callers to read garbage. Need to set w.*=0 before returning -2 in all four scan functions in zig/src/c_adapter.zig around lines 68-70 and equivalent locations.
