---
id: marky-luy.1
title: Add Miri CI step for unsafe arena patterns in parser/index
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-luy
---


The self-referential arena pattern in Ast and DocumentIndex uses unsafe (ptr::read, mem::forget, raw pointer casts to 'static). These should have Miri coverage in CI to validate soundness. Add a Miri job to ci.yml that runs tests for markymark-parser and markymark-index.
