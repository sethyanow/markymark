---
id: marky-v8y
title: 'incremental/mod.rs: signed arithmetic wraparound in adjust_range_after_edit'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

In markymark-lsp/src/incremental/mod.rs:138-166, adjust_range_after_edit and adjust_bytes_after_edit perform signed arithmetic then cast to unsigned, risking wraparound on negative results. Must use saturating arithmetic or clamp to 0. Flagged by CodeRabbit in PR #36.
