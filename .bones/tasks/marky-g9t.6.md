---
id: marky-g9t.6
title: Update LSP and MCP crates for arena lifetimes
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.5, marky-luy]
parent: marky-g9t
---





Update markymark-lsp and markymark-mcp to work with arena-allocated types. LSP state.rs holds documents with arenas. MCP runtime_engine.rs manages arena lifecycle. Convert/adapter code may need to copy arena strings into owned types for protocol responses. All transport crates must compile and pass tests.

Success: cargo test --workspace passes. Smoke tests (LSP + MCP) green.
