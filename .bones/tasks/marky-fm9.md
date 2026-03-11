---
id: marky-fm9
title: 'test: add test_export_index_includes_frontmatter for export-index MCP tool'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-khy]
---


## Context

marky-khy wired frontmatter/properties into DocumentIndex and updated export-index to include them in DocumentExport. However, the 7th required test (test_export_index_includes_frontmatter) was never added — test_no_properties_returns_empty was added instead.

The export-index implementation is correct at runtime_engine.rs:761-795:
- Frontmatter extracted and serialized as Vec<(String, Vec<String>)>
- Properties extracted and serialized as Vec<(String, String)>
- Both included in CoreOperationResult::DocumentExport{frontmatter, properties}

But there is NO automated test asserting that calling ExportIndex via CoreOperation on a document with frontmatter produces a DocumentExport result containing the frontmatter fields.

## What to add

In markymark-mcp/tests/runtime_engine_tests.rs (or runtime_engine.rs test mod):

test_export_index_includes_frontmatter:
  Setup: Create RuntimeEngine, index a markdown doc with YAML frontmatter:
    ---
    status: active
    tags: [rust, mcp]
    ---
    # My Doc
  Execute: CoreOperation::ExportIndex { uri: ..., realm: None }
  Assert: result is CoreOperationResult::DocumentExport
  Assert: frontmatter vec contains entry with key='status' and value=['active']
  Assert: frontmatter vec contains entry with key='tags' and value=['rust', 'mcp']
  Bug caught: frontmatter stored in DocumentIndex but not serialized in export output — would regress silently if serialization code removed

## File location

Look at existing tests in markymark-mcp/tests/runtime_engine_tests.rs for the ExportIndex pattern. Follow the same setup (temp dir, add_root, add document, execute operation).

## Success criteria
- 1 new test passing
- Test specifically verifies frontmatter IS in the DocumentExport result
- Test verifies list-valued frontmatter (tags array) is preserved
- All 879+ existing tests still passing
- No regressions
