---
id: marky-r9o
title: Extract SearchBlockText arm from execute() into engine/search_block_text.rs
status: closed
type: task
priority: 2
owner: Seth
parent: marky-nxc
---





## Context

`markymark-mcp/src/engine/mod.rs` execute() SearchBlockText arm (L632-679, ~48 lines) contains
inline query validation, kind filter parsing, search dispatch, and result mapping. Extract it
into a standalone async method on `RuntimeEngine` in a new `engine/search_block_text.rs` file.

This arm already uses `read_realm` (converted during marky-3yo). The extraction is purely
structural — move the body, add module declaration, replace arm with delegation call.

**Scope:** SearchBlockText extraction only. SemanticSearch (~84 lines, cfg-gated) is a separate
future task and the last remaining inline arm.

**Blocked by:** marky-6lc (closed — GetContentBlocks extraction, third precedent for the pattern)
**Unlocks:** After this, only SemanticSearch remains inline. Extracting both satisfies the
"execute() contains only delegating arms" criterion.

## Requirements

1. Extract SearchBlockText match arm body into `pub(super) async fn handle_search_block_text(&self, query: String, realm_name: Option<String>, kind_filter: Option<String>, limit: usize, include_text: bool) -> CoreOperationResult` on `RuntimeEngine`.
2. Define the method in a new file `engine/search_block_text.rs` using the `impl RuntimeEngine` split pattern (same as add_root.rs, content_blocks.rs).
3. Move query validation (empty/whitespace rejection), read_realm call, parse_block_kind call, search_block_text call, and BlockTextMatchResult map pipeline intact.
4. The match arm in execute() becomes a single delegation call.
5. No behavioral changes — existing tests must pass.

## Design

### Current structure (L632-679)

- **Query validation** (L639-644): Reject empty/whitespace queries with error message.
- **Realm lookup** (L646-649): `self.read_realm(realm_name.as_deref())` — already converted.
- **Kind filter parsing** (L652): `kind_filter.as_deref().and_then(helpers::parse_block_kind)` — converts wire string to BlockKind enum.
- **Search dispatch** (L654-659): `realm_data.index.search_block_text(query.trim(), block_kind_filter, limit, include_text)` — returns `(Vec<BlockTextMatch>, bool)`.
- **Map pipeline** (L661-671): Maps `BlockTextMatch` to `markymark_core::engine::BlockTextMatchResult` structs.
- **Return** (L673-678): `CoreOperationResult::BlockTextMatches { realm, query, matches, truncated }`.

### Imports needed in search_block_text.rs

```rust
use markymark_core::engine::{BlockTextMatchResult, CoreOperationResult};
use markymark_core::CoreError;

use super::{helpers, RuntimeEngine};
```

### After extraction, execute() SearchBlockText arm becomes

```rust
CoreOperation::SearchBlockText {
    query,
    realm: realm_name,
    kind_filter,
    limit,
    include_text,
} => {
    self.handle_search_block_text(query, realm_name, kind_filter, limit, include_text).await
}
```

## Implementation

### Step 1: Baseline — run tests via cargo MCP, confirm GREEN
### Step 2: Create `engine/search_block_text.rs` with `impl RuntimeEngine` block containing the SearchBlockText body
- Move query validation, read_realm call, parse_block_kind, search dispatch, and map pipeline
- Add `mod search_block_text;` to engine/mod.rs module declarations (alphabetical: after `search`, before `tests`)
- Cargo check
### Step 3: Replace SearchBlockText arm body in execute() with delegation call
- Cargo check
### Step 4: Verify imports in mod.rs — confirmed no-op
- `BlockTextMatchResult` is used via fully-qualified path (`markymark_core::engine::BlockTextMatchResult` at L663) only within the SearchBlockText arm — NOT via a `use` import. After extraction, no references remain in mod.rs. Nothing to remove.
- Cargo check (sanity confirmation)
### Step 5: Full verification — cargo test (default + all-features), cargo clippy

## Success Criteria

- [x] `handle_search_block_text` method exists in `engine/search_block_text.rs` as `impl RuntimeEngine`
- [x] SearchBlockText arm in execute() is a single delegation call
- [x] Query validation (empty/whitespace rejection) preserved in extracted method
- [x] Kind filter parsing via `helpers::parse_block_kind` preserved
- [x] Map pipeline to `BlockTextMatchResult` preserved
- [x] All tests pass (default features)
- [x] All tests pass (all features)
- [x] Clippy clean

## Anti-Patterns

- Do NOT extract SemanticSearch — separate future task (cfg-gated, complex multi-phase locking).
- Do NOT change the query validation logic, error messages, or BlockTextMatchResult structure.
- Do NOT introduce new abstractions (traits, generics).
- Do NOT use shell for cargo operations — use cargo MCP tools only.
- Do NOT omit the `pub(super)` visibility — method must be callable from mod.rs.
- Do NOT change `CoreEngine` trait or public API signatures.
- Do NOT skip the query validation move — the empty-query rejection IS part of the arm body and belongs in the extracted method, not left behind in execute().

## Key Considerations

- **Query validation belongs in the extracted method:** The empty/whitespace check (L639-644) is business logic specific to SearchBlockText — not generic execute() dispatch logic. Move it into handle_search_block_text.
- **read_realm already converted:** Unlike GetContentBlocks (marky-6lc), this arm already calls `self.read_realm()`. No conversion needed — just move the existing call.
- **BlockTextMatchResult import is fully-qualified:** Used as `markymark_core::engine::BlockTextMatchResult` at L663 — not in mod.rs's import list. Step 4 is a verified no-op.
- **Visibility pattern:** `pub(super)` on the method, following add_root.rs, content_blocks.rs precedent.
- **Module ordering:** `search_block_text` goes after `search` and before `tests` in the module declarations (alphabetical).
