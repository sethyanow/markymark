---
id: marky-fba
title: Extract AddRoot arm from execute() into engine/add_root.rs
status: open
type: task
priority: 2
parent: marky-nxc
---

## Context

`markymark-mcp/src/engine/mod.rs` execute() AddRoot arm (L483-603, ~120 lines) contains
a 4-phase locking protocol that's the most complex arm in the match. Extract it into a
standalone async method on `RuntimeEngine` in a new `engine/add_root.rs` file.

**Scope:** AddRoot extraction only. GetContentBlocks is a separate future task.

**Blocked by:** marky-3yo (closed — read_realm helper is in place)
**Unlocks:** GetContentBlocks extraction, and the "only delegating arms" criterion.

## Requirements

1. Extract AddRoot match arm body into `async fn handle_add_root(&self, realm: String, root: PathBuf) -> CoreOperationResult` on `RuntimeEngine`.
2. Define the method in a new file `engine/add_root.rs` using the `impl RuntimeEngine` split pattern.
3. The match arm in execute() becomes a single `self.handle_add_root(realm, root).await` call.
4. No behavioral changes — existing tests must pass.

## Design

### Current 4-phase structure (all moves to handle_add_root)

- **Phase 1:** Validate and register root — write lock, fast sync. Already delegates to `realm_ops::validate_and_register_root`.
- **Phase 2:** Collect + parse documents — no lock, I/O-bound. Uses `Md4cScanBackend`, `helpers::collect_documents`, `parse_structured`.
- **Phase 3:** Semantic embedding — read lock to get arc, then batch embed. `#[cfg(feature = "semantic-search")]` gated.
- **Phase 4:** Structural index update — write lock, includes race condition handling (root may have been removed during Phase 2/3).

### Imports needed in add_root.rs

```
use std::fs;
use std::path::PathBuf;
use markymark_core::engine::CoreOperationResult;
use markymark_core::scanner::Md4cScanBackend;
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, StructuredDocumentIndex};
use markymark_parser::structured::parse_structured;
use super::{helpers, realm_ops, RuntimeEngine};
```

Plus cfg-gated imports for semantic-search.

### After extraction, execute() AddRoot arm becomes

```rust
CoreOperation::AddRoot { realm, root } => {
    self.handle_add_root(realm, root).await
}
```

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Create `engine/add_root.rs` with `impl RuntimeEngine` block containing the AddRoot body
- Move the complete 4-phase logic including all cfg-gated sections
- Add `mod add_root;` to engine/mod.rs module declarations
- Cargo check
### Step 3: Replace AddRoot arm body in execute() with delegation call
- Cargo check
### Step 4: Clean up imports in mod.rs that may now be unused
- `fs`, `Md4cScanBackend`, `DocumentKind`, `parse_structured`, `DocumentIndex`, `StructuredDocumentIndex` may become unused if only AddRoot used them
- Check each with LSP findReferences before removing
- Cargo check
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [ ] `handle_add_root` method exists in `engine/add_root.rs` as `impl RuntimeEngine`
- [ ] AddRoot arm in execute() is a single delegation call (~1-2 lines)
- [ ] All 4 phases preserved in the extracted method (including cfg-gated semantic search)
- [ ] All tests pass (default features)
- [ ] All tests pass (all features)
- [ ] Clippy clean

## Anti-Patterns

- Do NOT extract GetContentBlocks — separate future task.
- Do NOT change the 4-phase locking protocol or race condition handling.
- Do NOT change public API signatures or the `CoreEngine` trait.
- Do NOT inline `validate_and_register_root` — keep the existing delegation to `realm_ops`.
- Do NOT use shell for cargo operations — use cargo MCP tools only.
- Do NOT remove cfg-gated semantic search code — it must move intact.
