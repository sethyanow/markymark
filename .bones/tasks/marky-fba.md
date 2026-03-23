---
id: marky-fba
title: Extract AddRoot arm from execute() into engine/add_root.rs
status: closed
type: task
priority: 2
owner: Seth
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

1. Extract AddRoot match arm body into `pub(super) async fn handle_add_root(&self, realm: String, root: PathBuf) -> CoreOperationResult` on `RuntimeEngine`.
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

```rust
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

**Also used via fully-qualified paths** (no import needed, just awareness):
- `markymark_index::parse_frontmatter_owned(&source)` (Phase 2, L508)
- `markymark_index::mask_frontmatter(&source)` (Phase 2, L509)
- `log::warn!(...)` macro (Phase 4, L579) — available without `use` in Rust 2018+

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
### Step 4: Verify imports in mod.rs — likely a no-op
- `fs`, `Md4cScanBackend`, `DocumentKind`, `parse_structured`, `DocumentIndex`, `StructuredDocumentIndex` are all also used by `index_root_into_realm()` (L263-313), so none will become unused after extraction
- Still verify with LSP findReferences before assuming — in case code has changed since this review
- Cargo check
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [x] `handle_add_root` method exists in `engine/add_root.rs` as `impl RuntimeEngine`
- [x] AddRoot arm in execute() is a single delegation call (~1-2 lines)
- [x] All 4 phases preserved in the extracted method (including cfg-gated semantic search)
- [x] All tests pass (default features)
- [x] All tests pass (all features)
- [x] Clippy clean

## Anti-Patterns

- Do NOT extract GetContentBlocks — separate future task.
- Do NOT change the 4-phase locking protocol or race condition handling.
- Do NOT change public API signatures or the `CoreEngine` trait.
- Do NOT inline `validate_and_register_root` — keep the existing delegation to `realm_ops`.
- Do NOT use shell for cargo operations — use cargo MCP tools only.
- Do NOT remove cfg-gated semantic search code — it must move intact.
- Do NOT omit the `pub(super)` visibility on `handle_add_root` — without it, the method is private to add_root.rs and uncallable from execute() in mod.rs. Phase 1 precedent: `realm/cross_doc.rs` uses `pub(super)` for split `impl` methods.

## Key Considerations (SRE Review)

- **Visibility pattern:** All `impl RuntimeEngine` methods in child modules need `pub(super)` to be callable from mod.rs. This follows the Phase 1 precedent where `realm/cross_doc.rs`, `realm/search.rs`, and `realm/journal.rs` use `pub(super)` for split `impl RealmIndex` methods.
- **AddRoot vs index_root_into_realm divergence:** The AddRoot arm (L512) uses `DocumentIndex::from_scan_with_frontmatter` while `index_root_into_realm` (L291) uses `DocumentIndex::from_scan_with_blocks` (which also calls `extract_raw_content_blocks`). This is existing behavior — do not "fix" or align these during extraction.
- **Self field access:** The extracted method accesses `self.state` (write + read locks across 4 phases). Since the new file's `impl RuntimeEngine` block has full access to all struct fields via `&self`, no additional plumbing is needed.
- **Race condition preservation (Phase 4, L562-589):** The root-still-present check and cfg-gated semantic cleanup must move intact. This is the most subtle part of the extraction — it handles the case where root was removed by another caller during Phase 2/3's lock-free I/O.

## Log

- [2026-03-23T10:23:56Z] [Seth] SRE refinement complete (13-category review). Key changes: (1) Added pub(super) visibility to handle_add_root signature — without it, method would be private to add_root.rs and uncallable from mod.rs. (2) Documented fully-qualified calls (markymark_index::parse_frontmatter_owned, mask_frontmatter, log::warn!) not in original import list. (3) Corrected Step 4 — all listed imports are shared with index_root_into_realm, so none become unused after extraction (likely no-op). (4) Added Key Considerations: visibility pattern, AddRoot vs index_root_into_realm parsing divergence, self field access, race condition preservation. (5) Added anti-pattern for visibility omission. All architecture claims verified accurate (line numbers, 4-phase structure, cfg-gated code). Assessment: APPROVE with changes applied.
