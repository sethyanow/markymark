---
id: marky-ltcu
title: 'Refactor realm.rs: split at 926 lines approaching hard stop'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

realm.rs is at 926 lines (hard stop is 1000). Split into submodules before it blocks feature work. Candidates: journal detection logic (~40L), structured doc ops (~80L), semantic index ops (~60L, feature-gated), find_uri helpers (~40L). Follow safe split pattern: (1) module dir, (2) extract types, (3) extract helpers, (4) tests — each step: edit→test→commit.
