---
id: marky-iqg
title: Extract SemanticSearch arm from execute() into engine/semantic_search.rs
status: open
type: task
priority: 2
parent: marky-nxc
---



## Context

`markymark-mcp/src/engine/mod.rs` execute() SemanticSearch arm (L361-445, 85 lines) is the
last inline arm containing business logic. It has dual cfg blocks (`#[cfg(not(feature =
"semantic-search"))]` and `#[cfg(feature = "semantic-search")]`), a three-phase lock protocol,
and does NOT use `read_realm` (intentional — Arc clone pattern conflicts with read_realm's
guard lifecycle).

Extract into a standalone async method on `RuntimeEngine` in a new `engine/semantic_search.rs`
file, following the `impl RuntimeEngine` split pattern established by add_root.rs,
content_blocks.rs, and search_block_text.rs.

**Scope:** SemanticSearch extraction only. After this, ALL execute() arms delegate — satisfying
the Phase 2 criterion "execute() contains only delegating arms."

**Blocked by:** marky-r9o (closed — SearchBlockText extraction, 4th precedent)
**Unlocks:** Phase 2 final criterion. After this, only Phase 3 (server.rs) and Phase 4
(low-severity cleanup) remain in the epic.

## Requirements

1. Extract SemanticSearch match arm body into `pub(super) async fn handle_semantic_search(&self, query: String, realm: Option<String>, top_k: usize, min_score: Option<f32>) -> CoreOperationResult` on `RuntimeEngine`.
2. Define the method in a new file `engine/semantic_search.rs` using the `impl RuntimeEngine` split pattern.
3. Move realm validation (non-cfg-gated, L371-378), `#[cfg(not(feature = "semantic-search"))]` NotImplemented block (L380-386), and `#[cfg(feature = "semantic-search")]` three-phase protocol (L388-444) intact into the method body.
4. The match arm in execute() becomes a single delegation call.
5. Use cfg-gated imports in the new file: types needed only under `semantic-search` feature must be `#[cfg(feature = "semantic-search")]`-gated.
6. No behavioral changes — existing tests must pass under both default and all-features.

## Design

### Current structure (L361-445)

- **Pattern destructure** (L361-365): `query`, `realm`, `top_k`, `min_score`.
- **Realm name resolution** (L367): `realm.unwrap_or_else(|| DEFAULT_REALM.to_string())` — uses `DEFAULT_REALM` constant from mod.rs.
- **Realm existence validation** (L371-378): Non-cfg-gated. Acquires `self.state.read().await`, checks `state.contains_key(&realm_name)`. Returns error if missing. Read guard dropped at block end.
- **cfg-off block** (L380-386): Suppresses unused variables, returns `CoreError::NotImplemented`.
- **cfg-on block** (L388-444):
  - **Empty query validation** (L390-395): Rejects empty/whitespace queries.
  - **Phase 1** (L399-423): Read lock → get realm_data → clone semantic Arc via `semantic_index_arc()` → get provider via `guard.provider()` → drop read lock.
  - **Phase 2** (L425-433): No lock → `provider.embed(&query).await` (slow: network/ONNX).
  - **Phase 3** (L435-443): `search::handle_semantic_search_with_embedding(...)` inside mutex (fast).

### Why no read_realm

The three-phase protocol intentionally avoids `read_realm` because:
1. `read_realm` returns a `RwLockReadGuard` that holds the lock for the caller's scope
2. Phase 1 needs to clone the Arc and provider, then DROP the lock before Phase 2
3. Holding the lock during Phase 2 (potentially slow embedding) would block all other realm access
4. The explicit lock-then-drop pattern is the correct design here

### Imports needed in semantic_search.rs

```rust
use markymark_core::engine::CoreOperationResult;
use markymark_core::CoreError;

use super::{search, RuntimeEngine, DEFAULT_REALM};
```

Plus cfg-gated imports for types only available under `semantic-search` feature. The executing
agent should determine exact cfg-gated imports by checking what types the Phase 1-3 code
references (semantic_index_arc return type, provider type, embed method).

### After extraction, execute() SemanticSearch arm becomes

```rust
CoreOperation::SemanticSearch {
    query,
    realm,
    top_k,
    min_score,
} => {
    self.handle_semantic_search(query, realm, top_k, min_score).await
}
```

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Create `engine/semantic_search.rs` with `impl RuntimeEngine` block
- Move the full SemanticSearch arm body: realm name resolution, realm existence validation, BOTH cfg blocks (not-feature and feature), three-phase lock protocol
- Determine cfg-gated imports empirically: write the non-cfg imports first, cargo check, add cfg-gated imports for any unresolved types
- Add `mod semantic_search;` to engine/mod.rs module declarations (alphabetical: after `search_block_text`, before the blank line/comment before `DEFAULT_REALM`)
- Cargo check
### Step 3: Replace SemanticSearch arm body in execute() with delegation call
- Cargo check
### Step 4: Verify mod.rs imports — check whether any cfg-gated imports at top of mod.rs are now unused
- L4-5: `#[cfg(feature = "semantic-search")] use std::sync::Arc;` — check if Arc is still used elsewhere in mod.rs (e.g., RuntimeEngine struct field, from_workspace_roots_with_provider)
- L12-13: `#[cfg(feature = "semantic-search")] use markymark_core::prelude::{EmbedError, EmbeddingProvider};` — check if used elsewhere in mod.rs
- Only remove imports that are genuinely unused after extraction
- Cargo check
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [ ] `handle_semantic_search` method exists in `engine/semantic_search.rs` as `impl RuntimeEngine`
- [ ] SemanticSearch arm in execute() is a single delegation call
- [ ] Non-cfg realm existence validation (L371-378 pattern) preserved in extracted method
- [ ] `#[cfg(not(feature = "semantic-search"))]` NotImplemented block preserved
- [ ] `#[cfg(feature = "semantic-search")]` three-phase lock protocol preserved
- [ ] Three-phase protocol does NOT use `read_realm` (intentional design — explicit lock/drop)
- [ ] All tests pass (default features)
- [ ] All tests pass (all features)
- [ ] Clippy clean

## Anti-Patterns

- Do NOT use `read_realm` in the extracted method — the three-phase lock protocol intentionally manages its own lock lifecycle. Using `read_realm` would hold the lock during Phase 2 (embedding), blocking all realm access.
- Do NOT merge the two cfg blocks — the `#[cfg(not(feature = "semantic-search"))]` and `#[cfg(feature = "semantic-search")]` blocks serve different purposes and must both be present.
- Do NOT change the three-phase lock protocol, error messages, or return types.
- Do NOT introduce new abstractions (traits, generics).
- Do NOT change `CoreEngine` trait or public API signatures.
- Do NOT cfg-gate the entire file or the method signature — only the body blocks and their specific imports are cfg-gated.
- Do NOT skip verifying which mod.rs imports become unused — prior extractions left imports unchanged because they were used elsewhere. This extraction may differ due to the cfg-gated types.
- Do NOT use shell for cargo operations — use cargo MCP tools only.

## Key Considerations

- **DEFAULT_REALM access**: The constant is defined in mod.rs at L30. Access from the new file via `super::DEFAULT_REALM`. It's a `&str` constant, not cfg-gated.
- **Module ordering**: `semantic_search` goes after `search_block_text` in the module declarations (alphabetical). The current order is: add_root, content_blocks, diagnostics, export, helpers, outline, realm_ops, references, search, search_block_text. Add `semantic_search` between `search` and `search_block_text`.
- **Visibility pattern**: `pub(super)` on the method, following all prior precedents.
- **cfg-gated imports in the new file**: Some types (e.g., `Arc`, `EmbeddingProvider`, `EmbedError`) are only available under `semantic-search` feature. Import them with `#[cfg(feature = "semantic-search")]` in the new file. The executing agent should determine the exact set by iterating: write code, cargo check, add missing cfg-gated imports.
- **Mod.rs import cleanup**: After extraction, check if `std::sync::Arc` (L5) and `EmbedError, EmbeddingProvider` (L13) are still used in mod.rs. `Arc` is likely still used in the `RuntimeEngine` struct (`provider: Option<Arc<dyn EmbeddingProvider>>`). `EmbedError` and `EmbeddingProvider` may or may not remain used in mod.rs — verify before removing.
