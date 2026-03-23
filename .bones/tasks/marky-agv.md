---
id: marky-agv
title: Deduplicate link edges in graph analysis
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #35 review (T2-3): Duplicate source->target link edges inflate hub scores and in-degree counts. Design decision needed: deduplicate by (source, target) pair, or document raw-count as intentional behavior.
