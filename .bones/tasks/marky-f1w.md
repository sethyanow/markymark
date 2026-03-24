---
id: marky-f1w
title: 'Task 1: FFI edit range plumbing — extend update signature, zero-value pass-through'
status: closed
type: task
priority: 2
owner: Seth
parent: marky-686
---




## Context

Phase 2 (marky-686) of Engine Pipeline v2 (marky-zsys). First task — pure FFI plumbing.

The Zig DocumentEngine's `update()` and its C export `marky_engine_update` currently take only
`(text, text_len)`. This task extends the FFI boundary to accept edit range parameters so that
future tasks (Task 2) can implement slug reuse for headings outside the edited region.

**No behavioral change.** The Zig side accepts the new parameters but ignores them — existing
`parseAll` pipeline runs identically. Zero-values (0/0/0) are the sentinel for "no range info."

**Blocked by:** nothing (first task in Phase 2)
**Unlocks:** Task 2 (Zig slug reuse logic — requires the FFI to carry edit range info)

## Requirements

From parent sub-epic marky-686:
- R3 (partial): `DocumentEngine::update()` FFI accepts optional edit range parameters

## Success Criteria

- [x] `marky_engine_update` C export in `exports.zig` takes 3 additional `u32` params: `edit_offset`, `edit_old_len`, `edit_new_len`
- [x] Zig `DocumentEngine.update()` signature accepts edit range params (ignored — passed through to no-op)
- [x] Rust extern block declares matching 6-param signature
- [x] Rust `EditRange` struct defined: `{ offset: u32, old_len: u32, new_len: u32 }`
- [x] Rust `DocumentEngine::update()` accepts `Option<EditRange>`, converts `None` to 0/0/0
- [x] All existing callers updated to pass `None` (LSP, MCP, 9 kernel tests, 3 Zig export tests + 5 Zig engine tests)
- [x] Test: `update(text, Some(EditRange { offset: 0, old_len: 0, new_len: 0 }))` produces same content hash as `update(text, None)`
- [x] Test: `update(text, Some(EditRange { offset: 100, old_len: 50, new_len: 75 }))` succeeds (non-zero values don't crash — verifies FFI param marshaling)
- [x] All existing tests pass (behavioral equivalence)

## Anti-Patterns

- NO changing parseAll or slug computation logic (that's Task 2)
- NO removing the zero-value sentinel contract (0/0/0 = no range info, defined by parent epic)
- NO adding edit range to `create()` (only `update()` — create has no "previous state")
- NO Rust-only signature change — Zig export MUST change in the same step (C ABI matches by symbol name only; linker won't catch param count mismatch, and extra args are silently ignored on x86-64/ARM64)
- NO `#[allow(unused)]` or suppression of edit range params on Rust side — they must be forwarded to FFI

## Implementation

### Step 1: RED — Write failing test for EditRange and new update() signature
**File:** `markymark-kernels/src/engine.rs` (tests module)
- Add `test_engine_update_with_edit_range` that creates an engine, then calls `engine.update("# New\n", Some(EditRange { offset: 0, old_len: 0, new_len: 0 }))`
- **Run:** `cargo nextest -p markymark-kernels -E 'test(edit_range)'`
- **Expected:** compile error — `EditRange` undefined, `update()` takes wrong number of args

### Step 2: GREEN — Define EditRange struct in Rust
**File:** `markymark-kernels/src/engine.rs`
- Add `pub struct EditRange { pub offset: u32, pub old_len: u32, pub new_len: u32 }` near the top (after KernelError or similar)
- Do NOT modify `update()` signature yet — just the struct definition
- **Run:** same test — still fails (update signature mismatch), but EditRange resolves

### Step 3: GREEN — Extend Zig DocumentEngine.update() signature
**File:** `zig/src/engine/document.zig`
- Modify `pub fn update(self: *DocumentEngine, text: []const u8)` to accept 3 additional params: `edit_offset: u32, edit_old_len: u32, edit_new_len: u32`
- Body unchanged — params are accepted but unused (annotate with `_ = edit_offset; _ = edit_old_len; _ = edit_new_len;` to suppress unused warnings)

### Step 4: GREEN — Extend C export in exports.zig
**File:** `zig/src/engine/exports.zig`
- Modify `marky_engine_update` export to accept 3 additional `u32` params
- Pass them through to `engine.update(slice, edit_offset, edit_old_len, edit_new_len)`
- Update 3 Zig tests (`engine_update_basic`, `engine_update_null_handle`, `engine_update_null_text_nonzero_len`) to pass `0, 0, 0` for the new params

### Step 5: GREEN — Extend Rust FFI extern block and update() wrapper
**File:** `markymark-kernels/src/engine.rs`
- Modify extern declaration: `fn marky_engine_update(handle, text, text_len, edit_offset: u32, edit_old_len: u32, edit_new_len: u32) -> i32`
- Modify `pub fn update(&mut self, text: &str, edit_range: Option<EditRange>) -> Result<(), KernelError>`:
  - Extract `(offset, old_len, new_len)` from `edit_range.unwrap_or(EditRange { offset: 0, old_len: 0, new_len: 0 })`
  - Pass all 6 args to the FFI call (both empty and non-empty text branches)

### Step 6: GREEN — Update all callers to pass None
**Files:**
- `markymark-lsp/src/state/mod.rs`: `build_markdown_index_via_engine` — `.update(&masked)` → `.update(&masked, None)`
- `markymark-mcp/src/engine/mod.rs`: MCP engine update path — `.update(&masked)` → `.update(&masked, None)`
- `markymark-kernels/src/engine.rs` tests (7 functions, 9 call sites): `test_engine_get_result_generation_increments`, `test_engine_content_hash_stable`, `test_engine_content_hash_changes`, `test_engine_content_hash_multibyte_utf8`, `test_engine_content_hash_repeated_updates_deterministic`, `test_engine_content_hash_redundant_headings`, `test_engine_content_hash_after_failed_update` — add `, None` to each `.update(...)` call
- Use LSP findReferences on `update` method to confirm no call sites missed

### Step 7: Verify — Run tests
- **Run:** `cargo nextest -p markymark-kernels` — kernel tests pass
- **Run:** `cargo nextest` — full workspace tests pass
- **Run:** `cargo clippy --workspace --all-targets` — no warnings from changes

### Step 8: Write zero-value equivalence test + non-zero marshaling test
**File:** `markymark-kernels/src/engine.rs` (tests module)
- `test_engine_update_edit_range_zero_equivalent`:
  Create two engines from same text. Update one with `None`, the other with `Some(EditRange { offset: 0, old_len: 0, new_len: 0 })`. Assert both produce same `content_hash()`.
- `test_engine_update_edit_range_nonzero_succeeds`:
  Create engine, update with `Some(EditRange { offset: 100, old_len: 50, new_len: 75 })`. Assert Ok — verifies non-zero values pass through FFI without error (even though Zig ignores them in Task 1).
- **Run:** `cargo nextest -p markymark-kernels -E 'test(edit_range)'`

### Step 9: Final verification and commit
- Full test suite: `cargo nextest 2>&1 | tail -5`
- Commit changes

## Key Considerations

- The `update()` signature change touches 12 call sites across 3 crates — mechanical but needs care
- Zig unused params must be explicitly discarded (`_ = param;`) to avoid compiler warnings
- The `text.is_empty()` branch in Rust's `update()` must also pass the edit range params to FFI
- EditRange should derive Debug, Clone, Copy, PartialEq, Eq for ergonomics
- **ABI safety:** Zig and Rust signatures must change atomically. C linker resolves by symbol name only — a Rust-side-only change (6 params declared, Zig export still has 3) links successfully but silently corrupts the stack / reads garbage for edit range params. The Zig export tests are the compile-time safety net: if the Zig export signature changes, the Zig tests must update to match, creating a forced coupling.
- **Bazel build:** No BUILD.bazel changes needed (no new crates/deps), but verify `bazel build //markymark-cli:markymark` still compiles after the signature change — Bazel builds Zig separately from Cargo.

### Adversarial Failure Catalog

Most failure categories are structurally eliminated for this task: params are u32 value types (no encoding ambiguity), ignored on Zig side (no input hostility), `&mut self` prevents concurrency (no temporal betrayal), and Rust's compiler catches missed callers (no partial updates). The genuine risks are documented below.

**Encoding Boundaries: Rust extern ↔ Zig C export parameter order**
- Assumption: Rust extern declaration and Zig export list params in identical order
- Betrayal: Param order swapped (e.g., `edit_old_len` before `edit_offset` on one side) — C ABI passes by position, not name. Values silently land in wrong registers.
- Consequence: Silent in Task 1 (Zig ignores all three). Task 2 reads edit_offset but receives edit_new_len — wrong byte range, wrong slug reuse, subtle data corruption.
- Mitigation: Non-zero marshaling test (Step 8) exercises the FFI path. Task 2's behavioral tests will catch order mismatches when params are actually consumed. Declare params in same order on both sides: `edit_offset, edit_old_len, edit_new_len`.

**Encoding Boundaries: Dual-branch FFI call in Rust wrapper**
- Assumption: Both `text.is_empty()` branches pass edit range params to FFI
- Betrayal: Agent updates the non-empty branch but forgets the empty-text branch (which passes `std::ptr::null(), 0` for text). The empty branch calls with 3 args, the non-empty with 6.
- Consequence: Compile error in Rust (both branches must match extern signature). **This is structurally caught.** But worth noting because the two branches look like they're doing different things.
- Mitigation: Compiler enforces. Both branches call `marky_engine_update` with identical param count.

**State Corruption: Zig `_ = param` discards edit range params**
- Assumption: `_ = param;` is temporary and Task 2 will replace with real usage
- Betrayal: Agent in Task 2 sees `_ = param;` and doesn't realize it needs replacement, or adds logic that uses some params but misses the `_` discard on others.
- Consequence: Edit range partially consumed — some params silently ignored in Task 2
- Mitigation: Add `// TODO(marky-686-task2): use edit range for slug reuse` comment next to each `_ =` discard. Task 2's success criteria explicitly require using these params.

**Categories not applicable (by construction):**
- Input Hostility: u32 value types, ignored in Task 1 — hostile values have no effect
- Temporal Betrayal: `&mut self` in Rust, single-threaded engine access — no concurrent calls possible
- Dependency Treachery: No new external dependencies — same parseAll pipeline
- Resource Exhaustion: 3 extra u32 stack values — negligible

## Log

- [2026-03-24T09:16:15Z] [Seth] Task 1 complete. Extended marky_engine_update FFI (Zig+Rust) with edit_offset/edit_old_len/edit_new_len u32 params. Zero-value sentinel (0/0/0) = no range info. Zig side accepts but ignores params (TODO for Task 2). All callers updated: LSP, MCP, 14 Zig tests, 9 Rust kernel tests. 3 new tests: basic edit range, zero-value equivalence, non-zero marshaling. 62 test suites, 0 failures, clippy clean. Commit d0c3d710.
