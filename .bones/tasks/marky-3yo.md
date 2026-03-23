---
id: marky-3yo
title: Eliminate realm-lookup boilerplate in execute() via read_realm helper
status: open
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

### Arms to refactor

GetOutline, SearchSymbols, FindReferences, Rename, RealmStats, DependencyGraph,
ExportIndex, SearchWorkspace, SearchForPattern, GraphAnalysis, GetDiagnostics,
SearchBlockText.

Note: SearchBlockText has input validation before the boilerplate — keep the
empty-query check before calling `read_realm`.

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Add `read_realm` method on RuntimeEngine, cargo check
### Step 3: Convert 12 arms to use helper, cargo check incrementally
### Step 4: Clean up unused imports, cargo check
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [ ] `read_realm` helper eliminates boilerplate in 12 read-lock arms
- [ ] SemanticSearch, GetContentBlocks, AddRoot unchanged
- [ ] All tests pass (default features)
- [ ] All tests pass (all features)
- [ ] Clippy clean

## Anti-Patterns

- Do NOT extract SemanticSearch, AddRoot, or GetContentBlocks — separate future tasks.
- Do NOT change public API signatures or the `CoreEngine` trait.
- Do NOT add traits or new public types.
- Do NOT use shell for cargo operations — use cargo MCP tools only.
