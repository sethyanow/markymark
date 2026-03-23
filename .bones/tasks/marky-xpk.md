---
id: marky-xpk
title: 'fix(rename): panic in find_wiki_link_heading_range on OOB slice'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
---

rename.rs:193-211 - &line[link_start..] panics if wl.range.start.character > line.len(). Add bounds check before slice.
