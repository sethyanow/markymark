---
id: marky-f1w
title: 'Task 1: FFI edit range plumbing — extend update signature, zero-value pass-through'
status: open
type: task
priority: 2
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

- [ ] `marky_engine_update` C export in `exports.zig` takes 3 additional `u32` params: `edit_offset`, `edit_old_len`, `edit_new_len`
- [ ] Zig `DocumentEngine.update()` signature accepts edit range params (ignored — passed through to no-op)
- [ ] Rust extern block declares matching 6-param signature
- [ ] Rust `EditRange` struct defined: `{ offset: u32, old_len: u32, new_len: u32 }`
- [ ] Rust `DocumentEngine::update()` accepts `Option<EditRange>`, converts `None` to 0/0/0
- [ ] All existing callers updated to pass `None` (LSP, MCP, 9 kernel tests, 3 Zig export tests)
- [ ] Test: `update(text, Some(EditRange { offset: 0, old_len: 0, new_len: 0 }))` produces same content hash as `update(text, None)`
- [ ] All existing tests pass (behavioral equivalence)

## Anti-Patterns

- NO changing parseAll or slug computation logic (that's Task 2)
- NO removing the zero-value sentinel contract (0/0/0 = no range info, defined by parent epic)
- NO adding edit range to `create()` (only `update()` — create has no "previous state")

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
- `markymark-lsp/src/state/mod.rs:218`: `.update(&masked)` → `.update(&masked, None)`
- `markymark-mcp/src/engine/mod.rs:335`: `.update(&masked)` → `.update(&masked, None)`
- 9 test call sites in `markymark-kernels/src/engine.rs` (lines 244, 255, 264, 307, 317, 319, 330, 356, 360): add `, None` to each `.update(...)` call

### Step 7: Verify — Run tests
- **Run:** `cargo nextest -p markymark-kernels` — kernel tests pass
- **Run:** `cargo nextest` — full workspace tests pass
- **Run:** `cargo clippy --workspace --all-targets` — no warnings from changes

### Step 8: Write zero-value equivalence test
**File:** `markymark-kernels/src/engine.rs` (tests module)
- `test_engine_update_edit_range_zero_equivalent`:
  Create two engines from same text. Update one with `None`, the other with `Some(EditRange { offset: 0, old_len: 0, new_len: 0 })`. Assert both produce same `content_hash()`.
- **Run:** `cargo nextest -p markymark-kernels -E 'test(edit_range)'`

### Step 9: Final verification and commit
- Full test suite: `cargo nextest 2>&1 | tail -5`
- Commit changes

## Key Considerations

- The `update()` signature change touches 12 call sites across 3 crates — mechanical but needs care
- Zig unused params must be explicitly discarded (`_ = param;`) to avoid compiler warnings
- The `text.is_empty()` branch in Rust's `update()` must also pass the edit range params to FFI
- EditRange should derive Debug, Clone, Copy, PartialEq, Eq for ergonomics
