---
id: marky-9cw
title: 'bug: UTF-16/byte-offset mismatch in completion.rs causes panic on multi-byte chars'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

completion.rs:82 slices a UTF-8 line using pos.character (UTF-16 code unit count per LSP spec) as a byte index. The bounds check compares UTF-16 count against byte length — both wrong. Any cursor position inside a multi-byte UTF-8 sequence (emoji, accented char) will panic at runtime. Fix: use existing lsp_position_to_byte_offset() from convert.rs:44-62 (already used correctly in incremental/mod.rs:51-52). Flagged by CodeRabbit as Critical in PR #36.
