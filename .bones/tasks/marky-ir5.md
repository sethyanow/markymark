---
id: marky-ir5
title: 'Refactor: split pattern.rs (952 lines, approaching 1000L HARD STOP)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

markymark-mcp/src/pattern.rs is 952 lines — approaching the 1000-line HARD STOP. Split into submodules similar to the engine/ and tools/ extraction done in marky-poe. Candidate splits: regex compilation, match iteration, result formatting.
