---
id: marky-e2j
title: 'completion.rs: UTF-16 position used as byte offset for line slicing'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

In markymark-lsp/src/state/completion.rs:76-82, pos.character (UTF-16 code unit index) is used directly as a byte offset to slice a UTF-8 line string. This will panic on multi-byte characters. Must use lsp_position_to_byte_offset or utf16_offset_to_byte_offset conversion. Flagged by CodeRabbit in PR #36.
