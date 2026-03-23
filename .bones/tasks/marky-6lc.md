---
id: marky-6lc
title: Extract GetContentBlocks arm from execute() into engine/content_blocks.rs
status: active
type: task
priority: 2
owner: Seth
parent: marky-nxc
---






## Context

`markymark-mcp/src/engine/mod.rs` execute() GetContentBlocks arm (L612-685, ~74 lines) contains
inline filter/map logic for querying content blocks by kind, heading, and block_id. Extract it
into a standalone async method on `RuntimeEngine` in a new `engine/content_blocks.rs` file.

Additionally, this arm still uses the OLD inline realm-lookup pattern (pre-marky-3yo). Convert
to `read_realm` during extraction.

**Scope:** GetContentBlocks extraction only. SearchBlockText and SemanticSearch are separate future tasks.

**Blocked by:** marky-fba (closed — AddRoot extraction established the `impl RuntimeEngine` split pattern in engine/)
**Unlocks:** The "execute() contains only delegating arms" criterion (requires further extractions of SearchBlockText and SemanticSearch after this).

## Requirements

1. Extract GetContentBlocks match arm body into `pub(super) async fn handle_get_content_blocks(&self, uri: DocumentUri, realm_name: Option<String>, kind_filter: Option<String>, heading_filter: Option<String>, block_id: Option<String>, include_text: bool) -> CoreOperationResult` on `RuntimeEngine`.
2. Define the method in a new file `engine/content_blocks.rs` using the `impl RuntimeEngine` split pattern (same as add_root.rs).
3. Convert the inline realm lookup to use `self.read_realm(realm_name.as_deref())`.
4. The match arm in execute() becomes a single delegation call.
5. No behavioral changes — existing tests must pass.

## Design

### Current structure (L612-685)

- **Realm lookup** (L620-626): Inline pattern — `unwrap_or(DEFAULT_REALM)`, `state.read().await`, `state.get(realm_key)`. Converts to `self.read_realm()`.
- **Document lookup** (L627-634): `realm_data.index.get_document(&uri)` with error on missing. Stays as-is.
- **Filter pipeline** (L639-663): Chain of `.filter()` closures checking kind_filter, heading_filter, and block_id against content blocks. Uses `helpers::block_kind_str()` and heading slug matching.
- **Map pipeline** (L664-682): Maps filtered blocks to `ContentBlockResult` structs. Uses `doc.block_text(b)` for optional text inclusion.
- **Return** (L684): `CoreOperationResult::ContentBlocks { uri, blocks }`.

### Imports needed in content_blocks.rs

```rust
use markymark_core::engine::{ContentBlockResult, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri};
use super::{helpers, RuntimeEngine};
```

### After extraction, execute() GetContentBlocks arm becomes

```rust
CoreOperation::GetContentBlocks {
    uri,
    realm: realm_name,
    kind_filter,
    heading_filter,
    block_id,
    include_text,
} => {
    self.handle_get_content_blocks(uri, realm_name, kind_filter, heading_filter, block_id, include_text).await
}
```

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Create `engine/content_blocks.rs` with `impl RuntimeEngine` block containing the GetContentBlocks body
- Convert realm lookup to `self.read_realm(realm_name.as_deref())`
- Keep document lookup and filter/map pipeline intact
- Add `mod content_blocks;` to engine/mod.rs module declarations (alphabetical: before `diagnostics`)
- Cargo check
### Step 3: Replace GetContentBlocks arm body in execute() with delegation call
- Cargo check
### Step 4: Verify imports in mod.rs — confirmed no-op
- `ContentBlockResult` is used via fully-qualified path (`markymark_core::engine::ContentBlockResult`) in the GetContentBlocks arm only — NOT via a `use` import at the top of mod.rs. After extraction, no references remain in mod.rs. Nothing to remove from the import list.
- Cargo check (sanity confirmation)
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [x] `handle_get_content_blocks` method exists in `engine/content_blocks.rs` as `impl RuntimeEngine`
- [x] GetContentBlocks arm in execute() is a single delegation call
- [x] Uses `read_realm` helper (old inline realm-lookup pattern eliminated)
- [x] Filter/map logic preserved (kind, heading, block_id filters + text inclusion)
- [x] All tests pass (default features)
- [x] All tests pass (all features)
- [x] Clippy clean

## Anti-Patterns

- Do NOT extract SearchBlockText or SemanticSearch — separate future tasks.
- Do NOT change the filter logic, ContentBlockResult structure, or error messages.
- Do NOT introduce new abstractions (traits, generics).
- Do NOT use shell for cargo operations — use cargo MCP tools only.
- Do NOT omit the `pub(super)` visibility — method must be callable from mod.rs.
- Do NOT change `CoreEngine` trait or public API signatures.
- Do NOT copy the inline realm-lookup pattern into content_blocks.rs — the read_realm conversion IS a deliverable of this task, not an optional cleanup. "Preserving behavioral equivalence" is not a reason to skip; read_realm produces identical error messages (verified by SRE).

## Key Considerations

- **read_realm conversion:** The current arm uses `realm_name.as_deref().unwrap_or(DEFAULT_REALM)` then `state.read().await` + `state.get(realm_key)`. The `read_realm` helper handles all of this including the default realm fallback. The error message format matches ("realm does not exist: X").
- **Document lookup is separate from realm lookup:** After `read_realm` provides `realm_data`, the document lookup (`realm_data.index.get_document(&uri)`) is a distinct step with its own error path. This two-step pattern (realm → document) stays in the extracted method.
- **ContentBlockResult import may move:** `ContentBlockResult` is currently imported in mod.rs via `markymark_core::engine`. If only GetContentBlocks uses it, the import may need to move to content_blocks.rs. Step 4 checks this.
- **Visibility pattern:** `pub(super)` on the method, following the add_root.rs and realm/cross_doc.rs precedent.

## Log

- [2026-03-23T10:42:42Z] [Seth] SRE review (fresh session, 13-category). APPROVE with 2 corrections applied: (1) Added anti-pattern blocking inline realm-lookup copy — read_realm conversion is a deliverable. (2) Clarified Step 4 as confirmed no-op — ContentBlockResult used via fully-qualified path, not imported. All architecture claims verified against current code: line numbers, read_realm error message equivalence, module ordering, add_root.rs precedent pattern.
