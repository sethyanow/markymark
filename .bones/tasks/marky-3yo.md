---
id: marky-3yo
title: Eliminate realm-lookup boilerplate in execute() via read_realm helper
status: closed
type: task
priority: 2
parent: marky-nxc
---




## Context

`markymark-mcp/src/engine/mod.rs` execute() has 12 read-lock arms that repeat the same
6-line realm-lookup boilerplate. Create a `read_realm` helper and refactor those 12 arms.

**Scope:** Boilerplate elimination only. AddRoot, GetContentBlocks, and SemanticSearch
are untouched — each is a separate future concern.

**Blocked by:** Nothing
**Unlocks:** Future extraction tasks for AddRoot and GetContentBlocks arms.

## Requirements

1. Create `read_realm` helper method on `RuntimeEngine` encapsulating the 6-line pattern.
2. Refactor the 12 standard read-lock arms to use it.
3. Leave SemanticSearch, GetContentBlocks, AddRoot, and write-lock arms unchanged.
4. No behavioral changes — existing tests must pass.

## Design

### The boilerplate (repeated 12 times)

```rust
let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
let state = self.state.read().await;
let Some(realm_data) = state.get(realm_key) else {
    return CoreOperationResult::Error(CoreError::Message(format!(...)));
};
```

### Helper approach

Use `tokio::sync::RwLockReadGuard::try_map` → `RwLockMappedReadGuard<'_, RealmData>`.
Returns resolved realm key string + the mapped guard. Takes `Option<&str>` for the
realm name (callers with `Option<String>` pass `.as_deref()`, callers with `String`
pass `Some(name.as_str())`).

**Caller error-handling pattern:** `execute()` returns `CoreOperationResult`, not
`Result<T, E>`, so the `?` operator won't work. Callers must use an explicit match:
```rust
let (realm_key, guard) = match self.read_realm(realm_name.as_deref()).await {
    Ok(v) => v,
    Err(e) => return e,
};
```

### Two sub-patterns among the 12 arms

**Pattern A (10 arms):** Realm field is `Option<String>` — defaults to `DEFAULT_REALM`:
GetOutline, SearchSymbols, FindReferences, Rename, ExportIndex, SearchWorkspace,
SearchForPattern, GraphAnalysis, GetDiagnostics, SearchBlockText.

**Pattern B (2 arms):** Realm field is `String` — no default needed:
RealmStats, DependencyGraph. Callers pass `Some(realm.as_str())` to the helper.

Note: SearchBlockText has input validation before the boilerplate — keep the
empty-query check before calling `read_realm`.

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Add `read_realm` method on RuntimeEngine, cargo check
### Step 3: Convert 12 arms to use helper, cargo check incrementally
### Step 4: Clean up unused imports, cargo check
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [x] `read_realm` helper eliminates boilerplate in 12 read-lock arms
- [x] SemanticSearch, GetContentBlocks, AddRoot unchanged
- [x] All tests pass (default features)
- [x] All tests pass (all features)
- [x] Clippy clean

## Anti-Patterns

- Do NOT extract SemanticSearch, AddRoot, or GetContentBlocks — separate future tasks.
- Do NOT change public API signatures or the `CoreEngine` trait.
- Do NOT add traits or new public types.
- Do NOT use shell for cargo operations — use cargo MCP tools only.
- Do NOT clone `RealmData` to avoid lifetime issues — the entire point of `try_map` is to hold
  the read lock via the mapped guard. Cloning data defeats the purpose and wastes memory.
- Do NOT refactor only the 10 `Option<String>` arms and skip the 2 `String` arms (RealmStats,
  DependencyGraph) — all 12 must use the helper.

## Key Considerations

- **Rust type safety provides structural guarantees:** `RwLockMappedReadGuard` holds the lock
  via ownership. The borrow checker prevents early drops. No runtime failure modes from the
  guard pattern — if it compiles, the lock lifetime is correct.
- **tokio 1.42+ with "full" features** — `RwLockReadGuard::try_map` confirmed available.
- **Adversarial planning found no significant failure modes.** This is a pure mechanical refactor
  with no new input surfaces, no new concurrency patterns, no external dependencies. The existing
  test suite is the complete verification.

## Log

- [2026-03-23T10:16:41Z] [Seth] Debrief: Pure mechanical refactor, all 12 arms converted. RwLockMappedReadGuard doesn't exist in tokio — try_map returns RwLockReadGuard<U> instead. Caller match-Ok-Err pattern needed because execute() returns CoreOperationResult not Result. Reflections: No surprises beyond the type name mismatch. Skeleton was accurate except for return type. Remaining Phase 2: AddRoot extraction (marky-fba scoped), GetContentBlocks extraction (future task).
