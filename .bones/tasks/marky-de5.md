---
id: marky-de5
title: 'fix(link_graph): deduplicate targets to prevent multi-edges'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

Duplicate IDs in targets slice inflate inbound_count, skew PageRank and orphan detection. File: zig/src/kernels/link_graph.zig lines 78-91. Fix: deduplicate targets using AutoHashMapUnmanaged before the append loop.
