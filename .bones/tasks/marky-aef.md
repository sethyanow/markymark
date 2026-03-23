---
id: marky-aef
title: 'Phase 2: Extract execute() boilerplate and long arms in engine/mod.rs'
status: open
type: task
priority: 2
parent: marky-nxc
---

## Context

`markymark-mcp/src/engine/mod.rs` is 881 lines. The `execute()` method (L316-875, 559 lines)
is a massive `match` dispatcher with 18 arms. 14 read-lock arms repeat the same 6-line
realm-lookup boilerplate. Two arms (`AddRoot` at 121 lines, `GetContentBlocks` at 74 lines)
contain substantial inline business logic. Pure decomposition — no behavioral changes.

**Blocked by:** Nothing (independent of Phase 1 — different crate)
**Unlocks:** Phases 3-4 can proceed independently, but closing this completes the P0 requirement.

**Existing module structure:** `engine/` already has `helpers.rs`, `realm_ops.rs`, `search.rs`,
`diagnostics.rs`, `export.rs`, `outline.rs`, `references.rs`, plus `tests/`. The extracted
arms follow the established delegation pattern (each arm calls a `module::handle_*` function).

**Test coverage:** 508-line unit test module at `engine/tests/mod.rs` plus 11 integration
test files in `tests/runtime_engine_tests/`. Pure refactoring — existing tests are the
regression tests.

## Requirements

1. Eliminate read-lock realm-lookup boilerplate via a `read_realm` helper method on
   `RuntimeEngine`, reducing each read-lock arm from 6 lines of boilerplate to a
   single helper call.
2. Extract `AddRoot` arm (L476-596, 121 lines, 4-phase async pipeline) into a standalone
   async method.
3. Extract `GetContentBlocks` arm (L747-820, 74 lines, inline filter/map) into a standalone
   function or method.
4. After extraction, `execute()` contains only short match arms that delegate — no inline
   business logic longer than ~10 lines.
5. No behavioral changes — existing tests must pass.

## Design

### Arm inventory (verified via LSP + code read)

**Standard read-lock arms (12 arms, boilerplate pattern):**
- GetOutline, SearchSymbols, FindReferences, Rename — simple: boilerplate + one handler call
- RealmStats, DependencyGraph, ExportIndex, SearchWorkspace, SearchForPattern,
  GraphAnalysis, GetDiagnostics — medium: boilerplate + handler call with more params
- SearchBlockText — boilerplate + input validation + handler call

**Non-standard read-lock arm:**
- SemanticSearch (L345-428, 84 lines) — complex multi-phase lock pattern with separate
  read/unlock/embed/mutex cycles. Does NOT use the standard boilerplate — skip for
  `read_realm` helper. Leave as-is.

**Large read-lock arm to extract:**
- GetContentBlocks (L747-820, 74 lines) — realm lookup + document lookup + inline
  filter/map logic building ContentBlockResult vec.

**Write-lock arms (leave as-is):**
- CreateRealm (11 lines), DestroyRealm (3 lines), RemoveRoot (4 lines) — short delegates
- AddRoot (121 lines) — extract

### `read_realm` helper design

The boilerplate pattern:
```
let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
let state = self.state.read().await;
let Some(realm_data) = state.get(realm_key) else {
    return CoreOperationResult::Error(CoreError::Message(format!(...)));
};
```

Use `tokio::sync::RwLockReadGuard::try_map` to produce a
`RwLockMappedReadGuard<'_, RealmData>` that holds the lock while providing `&RealmData`.
The helper is an async method on `RuntimeEngine` returning
`Result<(String, RwLockMappedReadGuard<'_, RealmData>), CoreOperationResult>`.

Signature (approximate — executing agent decides exact design):
```rust
async fn read_realm(&self, realm_name: Option<&str>)
    -> Result<(String, RwLockMappedReadGuard<'_, RealmData>), CoreOperationResult>
```

Some arms use `realm_name: Option<String>` (owned), others `Option<&str>`. The helper
takes `Option<&str>` and callers convert as needed. The returned `String` is the resolved
realm key (needed by some arms for return values).

### AddRoot extraction

The AddRoot arm has a 4-phase pipeline:
1. Validate and register root (write lock, fast sync) — already delegated to `realm_ops::validate_and_register_root`
2. Collect + parse documents (no lock, I/O-bound)
3. Semantic embedding (no outer lock, slow network I/O) — cfg-gated
4. Structural index update (write lock, fast in-memory ops)

Extract to an async method on `RuntimeEngine` (needs `&self` for `self.state` and
`self.provider`). Signature:
```rust
async fn handle_add_root(&self, realm: String, root: PathBuf) -> CoreOperationResult
```

### GetContentBlocks extraction

The inline logic does: realm lookup → document lookup → filter by kind/heading/block_id →
map to ContentBlockResult. Extract to a standalone function in `helpers.rs` or a new module.
It only needs `&RealmIndex`, `&DocumentUri`, and the filter params.

## Implementation

### Step 1: Baseline verification
- `cargo nextest -p markymark-mcp` — confirm GREEN, record test count
- `wc -l markymark-mcp/src/engine/mod.rs` — confirm 881

### Step 2: Implement read_realm helper
- Add `read_realm` async method on `RuntimeEngine` (in the existing `impl RuntimeEngine` block)
- Uses `tokio::sync::RwLockReadGuard::try_map` to map `state.get(realm_key)` into a
  `RwLockMappedReadGuard<'_, RealmData>`
- Returns `Result<(String, RwLockMappedReadGuard<'_, RealmData>), CoreOperationResult>`
- `cargo check -p markymark-mcp` — compiles (unused method warning expected)

### Step 3: Refactor read-lock arms to use read_realm
- Convert 12 standard read-lock arms (GetOutline, SearchSymbols, FindReferences, Rename,
  RealmStats, DependencyGraph, ExportIndex, SearchWorkspace, SearchForPattern, GraphAnalysis,
  GetDiagnostics, SearchBlockText) from 6-line boilerplate to `let (realm_key, realm_data) = self.read_realm(...).await?;`
- Leave SemanticSearch and GetContentBlocks untouched in this step
- `cargo check -p markymark-mcp` after every 3-4 arms to catch issues incrementally

### Step 4: Extract handle_add_root
- Create async method `handle_add_root(&self, realm: String, root: PathBuf) -> CoreOperationResult`
  on `RuntimeEngine`
- Move the 4-phase pipeline body from the AddRoot arm into this method
- AddRoot arm becomes: `CoreOperation::AddRoot { realm, root } => self.handle_add_root(realm, root).await`
- `cargo check -p markymark-mcp`

### Step 5: Extract handle_get_content_blocks
- Create a function (in helpers.rs or inline as a method) that takes `&RealmIndex`, `&DocumentUri`,
  filter params, and returns `CoreOperationResult`
- Move the filter/map logic from the GetContentBlocks arm into this function
- GetContentBlocks arm becomes: boilerplate + doc lookup + function call
- Apply `read_realm` to GetContentBlocks arm as well
- `cargo check -p markymark-mcp`

### Step 6: Clean up imports
- Remove any now-unused imports from mod.rs
- `cargo check -p markymark-mcp` — clean, no warnings

### Step 7: Full verification
- `wc -l markymark-mcp/src/engine/mod.rs` — significant reduction from 881
- `cargo nextest -p markymark-mcp` — all tests pass, same count as Step 1
- `cargo nextest -p markymark-mcp --all-features` — all tests pass
- `cargo clippy -p markymark-mcp --all-targets` — clean
- Commit and push

## Success Criteria

- [ ] `read_realm` helper on RuntimeEngine eliminates realm-lookup boilerplate in 12 read-lock arms
- [ ] SemanticSearch arm intentionally unchanged (complex multi-phase lock pattern)
- [ ] `AddRoot` arm extracted to a standalone async method — arm body is a single delegation call
- [ ] `GetContentBlocks` arm extracted — inline filter/map logic moved to a function
- [ ] `execute()` match arms are all short delegations — no inline business logic >10 lines
      (exception: SemanticSearch, which has a unique lock pattern)
- [ ] `cargo nextest -p markymark-mcp` passes (same test count as baseline)
- [ ] `cargo nextest -p markymark-mcp --all-features` passes
- [ ] `cargo clippy -p markymark-mcp --all-targets` clean

## Anti-Patterns

- Do NOT change any public API signatures or `CoreEngine` trait — callers must not need updates.
- Do NOT extract SemanticSearch — its multi-phase lock/unlock/embed pattern is fundamentally
  different from the standard boilerplate. Forcing it into the helper would add complexity.
- Do NOT add traits or new public types — `read_realm` is a private helper method.
- Do NOT move write-lock arms (CreateRealm, DestroyRealm, RemoveRoot) — they're already
  short delegations to `realm_ops`.
- Do NOT skip incremental `cargo check` — verify after each extraction step.
- Do NOT combine with feature work — pure refactoring only.
- Do NOT target a specific line count — the goal is structural (short arms), not numeric.

## Key Considerations

- **TDD escape hatch applies:** Pure structural refactoring. Existing tests (508 unit +
  11 integration test files) are the regression tests. No new tests needed.
- **`tokio::sync::RwLockReadGuard::try_map` lifetime:** The mapped guard borrows from
  `self.state`. The executing agent must verify the tokio version supports `try_map` —
  check `Cargo.toml` for the tokio version before implementing.
- **`AddRoot` uses `self` fields:** The extracted method needs `&self` (for `self.state`
  and `self.provider`). It's a method on `RuntimeEngine`, not a standalone function.
- **`GetContentBlocks` reads the document AND the index:** The extracted function needs
  `&RealmIndex` and `&DocumentUri` plus filter params. The document lookup
  (`realm_data.index.get_document(&uri)`) stays in the arm or moves into the function.
- **SearchBlockText has input validation before boilerplate:** The empty-query check at
  L832-836 runs before the realm lookup. The executing agent should keep this validation
  before calling `read_realm`, not embed it in the helper.
- **Some arms use `realm_name: Option<String>` while others use `realm: String`:** Arms
  with `Option<String>` pass `.as_deref()`, arms with `String` pass `Some(name.as_str())`.
  The helper's `Option<&str>` parameter handles both.

## Log

- [2026-03-23T04:15:00Z] [Seth] Task scoped from marky-nxc Phase 2 during executing-plans debrief. Full codebase verification: execute() is 559 lines (L316-875), 18 arms total (14 read-lock with standard boilerplate, 1 non-standard read-lock (SemanticSearch), 1 large read-lock (GetContentBlocks), 4 write-lock). AddRoot is 121 lines (L476-596). Module already has helpers.rs, realm_ops.rs, search.rs etc.
