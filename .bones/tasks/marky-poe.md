---
id: marky-poe
title: 'refactor: split runtime_engine.rs (1315L) and lib.rs (1005L) into submodules'
status: closed
type: task
priority: 0
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal
Split two oversized files in markymark-mcp:
- markymark-mcp/src/runtime_engine.rs: 1246 lines (HARD STOP violation)
- markymark-mcp/src/lib.rs: 779 lines (will grow with more tools)

## Plan

### Split runtime_engine.rs → engine/ submodule
- markymark-mcp/src/engine/search.rs — SearchSymbols, SemanticSearch, SearchWorkspace (move from search.rs)
- markymark-mcp/src/engine/graph.rs — DependencyGraph, build_dependency_graph
- markymark-mcp/src/engine/export.rs — ExportIndex, dto helpers
- markymark-mcp/src/engine/realm_ops.rs — CreateRealm, DestroyRealm, AddRoot, RemoveRoot, RealmStats
- markymark-mcp/src/engine/references.rs — FindReferences, Rename
- markymark-mcp/src/engine/outline.rs — GetOutline
- markymark-mcp/src/engine/mod.rs — RuntimeEngine struct, execute(), helpers (DEFAULT_REALM, index_root, unindex_root, fuzzy helpers) under 500 lines
- Remove runtime_engine.rs, replace with engine/ directory

### Split lib.rs → smaller tools/ submodule
- markymark-mcp/src/tools/search.rs — search-symbols, semantic-search, search-workspace tool handlers
- markymark-mcp/src/tools/outline.rs — get-outline, export-index handlers
- markymark-mcp/src/tools/refs.rs — find-references, rename handlers
- markymark-mcp/src/tools/realm.rs — create-realm, destroy-realm, add-root, remove-root, realm-stats handlers
- markymark-mcp/src/tools/mod.rs — re-exports
- markymark-mcp/src/lib.rs — server struct, list_tools, serve_stdio, helper fns only (under 300 lines)

## Success Criteria
- [ ] No file in markymark-mcp/src exceeds 500 lines
- [ ] All existing 842 workspace tests pass (no regressions)
- [ ] cargo clippy clean
- [ ] cargo fmt clean
