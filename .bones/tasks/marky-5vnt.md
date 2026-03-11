---
id: marky-5vnt
title: 'Zig engine: slug truncation returns empty + processLeafBlock silent catch {}'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

Two related issues in the Zig document engine pipeline:

**1. Slug truncation returns empty string (document.zig:396-404)**
slugifyText returns "" for ALL negative slugify() return codes, including -2 (truncated).
When rc == -2, the buffer contains output_cap (512) valid slug bytes. Fix: return out[0..512]
on -2, keep "" only for true errors (-1).

Impact: headings with >512-byte slugs get empty slugs, breaking anchor navigation and
creating collision chains ("", "-1", "-2").

**2. processLeafBlock silent catch {} (inlines.zig:38-40)**
Residual from PR#39 review. marky-0mr.6 fixed catch {} in collectEmphasisDelimiters but
missed the two catch {} in processLeafBlock:
  self.buffer.append(self.allocator, '\n') catch {};
  self.buffer.appendSlice(self.allocator, ...) catch {};

Function already returns Parser.Error!void, so changing to try is straightforward.

Impact: OOM during line merging produces garbled inline processing (missing lines,
wrong break detection).

Source: Codex + CodeRabbit review of PR #40
