---
id: marky-oiv
title: 'PR #34 deferred nitpicks: multi-edit coords, doc mismatch, saturating math'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

Deferred from PR #34 triage. Three low-priority items:
1. Multi-edit coordinate space mismatch in _affected_by_edits (incremental/mod.rs:251) - pre-existing, single-edit is fine
2. find_prose_edit_pos docs vs impl mismatch on backtick/tilde exclusion (parser/src/lib.rs:130) - bench utility only
3. Saturating arithmetic for adjust_bytes_after_edit (incremental/mod.rs:157) - defensive, tree-sitter edits always valid
