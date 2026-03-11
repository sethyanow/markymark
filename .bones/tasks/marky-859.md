---
id: marky-859
title: 'fix(link_graph): deduplicate targets to prevent multi-edges'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

link_graph.zig:78-91 - duplicate IDs in targets inflate inbound_count, skew PageRank/orphan detection. Deduplicate before loop.
