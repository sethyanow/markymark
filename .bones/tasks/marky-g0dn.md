---
id: marky-g0dn
title: 'Incremental merge: affected_by_edits misses new entries from large insertions'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

All 4 incremental extractors (xml_tags, wiki_links, markdown_links, blocks) use range_within_neighbor_window with old_end_byte + 100 to filter new entries in the merge step. For insertions larger than 100 bytes, new entries deep inside the inserted text have post-edit byte offsets beyond the window, so they're dropped from the merged result. The affected_by_edits functions need to also consider the edit's new_end_byte when testing new-AST entries. Affects: xml_tag_affected_by_edits, wiki_link_affected_by_edits, markdown_link_affected_by_edits, block_affected_by_edits. Source: CodeRabbit review of PR #38.
