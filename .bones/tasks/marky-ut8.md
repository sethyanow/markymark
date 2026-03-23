---
id: marky-ut8
title: 'Phase 4.2: Migrate from_scan_with_frontmatter production callers + delete from_ast.rs'
status: open
type: task
priority: 2
parent: marky-0xtn
---

## Context

- Phase 4.1 (marky-llj) added `from_text()` and migrated all 18 from_ast callers. from_ast now has zero callers.
- `from_scan_with_frontmatter` still has 5 callers (excl. definition): 2 production fallbacks (MCP + LSP), 1 MCP AddRoot handler, 1 LSP test, 1 dead from_ast.rs call.
- The MCP AddRoot handler (`CoreEngine::execute` at mod.rs:589-624) duplicates `index_root_into_realm` using from_scan — a Phase 3 gap.
- `from_scan` (no frontmatter) has 13 callers in test/bench code — separate task scope.

**Blocked by:** marky-llj (closed — from_text available)
**Unlocks:** from_ast.rs deletion, from_scan_with_frontmatter caller elimination from production. After this, from_scan_with_frontmatter callers are only in from_scan.rs itself (called by the from_scan_inner chain) and from_scan test code.

## Requirements

- R9 (from epic): DocumentIndex::from_text() convenience function replaces from_ast/from_scan/from_blob in all tests.
- R4 (from epic): MCP batch path uses persistent engines + from_engine_result (no from_scan).
- This task covers: migrate from_scan_with_frontmatter production callers to from_text/engine path, delete from_ast.rs.

## Implementation

1. Write equivalence test: in `markymark-mcp/src/engine/tests/`, verify that `DocumentIndex::from_text(text)` produces the same headings, tags, links, and frontmatter as the current `fallback_scan_with_frontmatter(text)` for a mixed document. Run → should pass.

2. Replace MCP `fallback_scan_with_frontmatter` body (mod.rs:262-265):
   - Change body to: `DocumentIndex::from_text(text)`
   - Remove `Md4cScanBackend` import if no other caller uses it in this function's scope.

3. Replace MCP `AddRoot` handler (mod.rs:589-650): The inline scan loop duplicates `index_root_into_realm`. Replace the Phase 2 collect+parse+add logic with a call to `index_root_into_realm(root, realm_data).await`. This already uses persistent engines and handles structured docs.
   - Key: need to acquire a mutable reference to the realm data. Check the existing locking pattern.
   - The `index_root_into_realm` function takes `(&Path, &mut RealmData)` and handles both markdown and structured docs.

4. Replace LSP `fallback_scan_with_frontmatter` body (state/mod.rs:164-168):
   - Same swap to: `DocumentIndex::from_text(text)`
   - Remove `Md4cScanBackend` import if no longer needed.

5. Migrate LSP test (state_tests.rs:747):
   - Replace `from_scan_with_frontmatter(&masked, &Md4cScanBackend, fm, aliases)` with `DocumentIndex::from_text(text)`.
   - Remove frontmatter parse+mask setup lines since from_text handles them internally.

6. Delete `from_ast.rs`:
   - Remove `markymark-index/src/document/from_ast.rs`
   - Remove `mod from_ast;` from `document/mod.rs:6`

7. Update compile_fail doctest in `document/mod.rs:131-141`:
   - Currently uses `Parser` + `from_ast` to demonstrate arena lifetime safety
   - Replace with `from_text("# Title")` — same lifetime constraint, no Parser dependency

8. Verify: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, zero from_scan_with_frontmatter callers in production code (MCP + LSP).

## Key Considerations

- `fallback_scan_with_frontmatter` is a fallback for engine failures. Replacing its body with `from_text` changes the fallback from scan→engine rather than scan→scan. This is correct: `from_text` creates an ephemeral engine, and if the persistent engine failed, an ephemeral one may succeed (fresh state, no stale handle). If `from_text` also fails, it panics — but this is acceptable because the fallback is last resort before stale cache.
- The MCP AddRoot handler at mod.rs:589-650 needs careful inspection: it handles realm locking and both markdown + structured docs. `index_root_into_realm` already does this. Check that the locking pattern is compatible.
- After this task, `from_scan_with_frontmatter` will still be referenced by from_scan's internal chain and test code. The from_scan.rs file itself won't be deletable yet — that's a follow-up task.
- The compile_fail doctest change is load-bearing: it proves arena lifetime safety. Verify the doctest still fails to compile for the right reason after the change.

## Success Criteria

- [ ] MCP `fallback_scan_with_frontmatter` uses `from_text` (no from_scan_with_frontmatter)
- [ ] MCP `AddRoot` handler uses `index_root_into_realm` (no inline scan loop)
- [ ] LSP `fallback_scan_with_frontmatter` uses `from_text` (no from_scan_with_frontmatter)
- [ ] LSP test migrated to `from_text`
- [ ] `from_ast.rs` deleted — file removed, mod declaration removed
- [ ] compile_fail doctest updated to use `from_text`
- [ ] cargo test --workspace passes
- [ ] cargo clippy --workspace -- -D warnings passes

## Anti-Patterns

- No keeping from_scan_with_frontmatter in production fallback — from_text is the replacement
- No deleting from_scan.rs yet — it still has 13 test callers that are a separate task
- FORBIDDEN: changing fallback_scan_with_frontmatter to return Result — it's a last-resort fallback, panic on failure is acceptable
- No removing the stale index cache fallback — that stays (it precedes the scan fallback in the fallback chain)
