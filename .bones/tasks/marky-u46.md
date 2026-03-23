---
id: marky-u46
title: 'fix(rename): panic in find_markdown_link_anchor_range on OOB slice'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
---

rename.rs:222-240 - same issue as wiki link variant. &line[link_start..] panics if ml.range.start.character > line.len().
