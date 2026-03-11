---
id: marky-43i
title: Optimize remove_from_cross_doc_indexes to O(doc size) instead of O(total indexed)
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #13 CodeRabbit review: remove_from_cross_doc_indexes scans ALL slug/block/tag buckets on every document change, making it O(total indexed items). In the LSP this runs on every edit (remove+add). Fix: look up the existing DocumentIndex for the URI and delete only the specific slugs/block-ids/tags contributed by that document, or maintain a per-doc reverse index. File: markymark-index/src/realm.rs:136
