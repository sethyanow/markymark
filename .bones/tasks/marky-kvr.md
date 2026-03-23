---
id: marky-kvr
title: find-references fails on structured docs (JSON/YAML/TOML)
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

**Summary:**
`find-references` returns a 'document is not indexed' error when called on structured documents (TOML, YAML, JSON), even though `get-outline`, `export-index`, and `search-symbols` all work correctly on the same files.

**Steps to reproduce:**
1. Create a realm and add a root containing structured docs
2. Verify `get-outline` works on a structured file (e.g. `Cargo.toml`):
   ```
   get-outline(uri: 'file:///path/to/Cargo.toml')
   → Returns full key-path hierarchy ✅
   ```
3. Call `find-references` on the same file:
   ```
   find-references(uri: 'file:///path/to/Cargo.toml', line: 0, character: 5)
   → Error: 'document is not indexed' ❌
   ```

**Expected behavior:**
`find-references` should either work on structured doc key paths, or return an empty result set with no error.

**Actual behavior:**
Returns error: `{"error":{"code":"core_error","message":"document is not indexed: file:///Volumes/code/markymark/Cargo.toml"}}`

**Affected tools:**
- `find-references` ❌
- `get-outline` ✅ works
- `export-index` ✅ works  
- `search-symbols` ✅ works
- `realm-stats` ✅ counts structured docs

**Root cause hypothesis:**
`find-references` likely checks a markdown-only document index rather than the unified index that includes structured docs. The structured doc indexing pipeline may register documents in the symbol/outline store but not in the reference-resolution store.
