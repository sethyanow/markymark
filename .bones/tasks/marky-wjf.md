---
id: marky-wjf
title: 'incremental: neighbor-window can miss insertions in large gaps between entries'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

In markymark-lsp/src/incremental/mod.rs:170-555, the neighbor-window check (100 byte window) can miss edits inserted in large gaps between consecutive old entries. Need a gap-detection fallback: if any edit.start_byte lies strictly between prev entry end_byte and next entry start_byte, trigger re-extraction. Flagged by CodeRabbit in PR #36.
