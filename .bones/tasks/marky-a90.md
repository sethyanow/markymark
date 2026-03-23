---
id: marky-a90
title: 'P2 refactor: split markymark-mcp/tests/runtime_engine_tests.rs below 1000 LOC (currently 1916)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

runtime_engine_tests.rs is 1916 lines — nearly 2x the 1000-line hard stop. Split into logical test modules (e.g. by MCP tool group: search, outline, symbols, realm management, graph analysis). Each resulting file should be under 500 lines. Pattern: create tests/ subdirectory with mod.rs re-exporting submodules, move test groups, verify cargo nextest -p markymark-mcp passes after each move.
