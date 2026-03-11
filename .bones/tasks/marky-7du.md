---
id: marky-7du
title: Remove unused bumpalo dependency
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

bumpalo is declared in workspace Cargo.toml and markymark-index/Cargo.toml but never used in source code (no imports, no Arena instances). The README previously claimed 'Arena allocation for O(1) cleanup' but this was never implemented. Remove the dep from both Cargo.toml files.
