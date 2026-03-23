---
id: marky-3w5
title: 'Refactor document.rs: split into submodules (approaching 1000-line limit)'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-b6v]
---


document.rs is at 857 lines. After marky-b6v restores from_scan (~120 lines) and scan_tests (~165 lines), it will reach ~1140 lines — past the 1000-line HARD STOP (learned rule-004). Split into submodules: types.rs (entry structs), build.rs (from_ast, from_scan constructors), helpers.rs (slugify, dedup_slug, build_toc, build_outline, byte_offset helpers), mod.rs (DocumentIndex public API + re-exports). Must happen immediately after marky-b6v lands.
