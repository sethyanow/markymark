---
id: marky-xh5
title: 'Refactor incremental.rs: split test module into separate file'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

incremental.rs grew to 1093 lines (568 production + 525 tests) after migrating incremental tests from state/mod.rs. Exceeds the 1000-line HARD STOP. Split the test module into a separate tests/incremental_unit.rs or incremental/tests.rs file.
