---
id: marky-ejt
title: 'Refactor zig/src/shared/similarity.zig: split tests into separate file'
status: open
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

similarity.zig is 530 lines (500-line threshold breached after adding PR29 triage tests in bda7df9). Split tests out to zig/src/shared/similarity_test.zig or equivalent test file. The implementation is ~210 lines; tests are ~320 lines. Zig supports @import for test files.
