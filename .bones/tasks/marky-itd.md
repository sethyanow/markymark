---
id: marky-itd
title: 'Refactor markymark-mcp/lib.rs: extract tools into submodules (919→<500 lines)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

## Design

lib.rs is at 919 lines after adding search-for-pattern tool (marky-59b). Project rule #2 says 1000-line hard stop; 919 is within 80 lines. Extract tool handler methods into per-feature modules to bring lib.rs under 500 lines.

## Extraction targets
- tools/outline.rs: get_outline_tool (~40 lines)
- tools/symbols.rs: search_symbols_tool, semantic_search_tool (~80 lines)
- tools/references.rs: find_references_tool, rename_tool (~80 lines)
- tools/realm.rs: create_realm_tool, destroy_realm_tool, add_root_tool, remove_root_tool, realm_stats_tool (~160 lines)
- tools/export.rs: export_index_tool (~90 lines)
- tools/search.rs: search_workspace_tool, search_for_pattern_tool (~130 lines)
- Keep: struct definition, impl new, list_tools, serve_stdio, helper fns (~200 lines)

lib.rs after extraction: ~200 lines

## Success Criteria
- [ ] lib.rs under 500 lines
- [ ] All existing tests still pass
- [ ] Tool behavior unchanged
- [ ] cargo clippy clean, cargo fmt clean
