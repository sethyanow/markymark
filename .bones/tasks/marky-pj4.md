---
id: marky-pj4
title: 'rename.rs: closing-tag Position::new can underflow on short tag names'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

In markymark-lsp/src/state/rename.rs:155-168, the closing-tag start calculation can underflow when xml.tag_name.len() >= xml.range.end.character - 1. Needs checked/saturating subtraction. Flagged by CodeRabbit in PR #36.
