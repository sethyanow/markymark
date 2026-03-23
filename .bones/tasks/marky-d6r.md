---
id: marky-d6r
title: Audit EmbeddingIndex unsafe impl Sync soundness
status: closed
type: task
priority: 3
owner: Seth
---





## Context

`EmbeddingIndex` (markymark-kernels/src/embed.rs:101) has `unsafe impl Sync` with a documented
invariant: only safe under external RwLock. The safety comment at embed.rs:91-99 is explicit.

**Invariant verification:** The call chain enforces the invariant:
- `EmbeddingIndex` → `SemanticIndex` (wraps it, markymark-index/src/semantic/mod.rs:23)
- `SemanticIndex` → `RealmIndex` stores as `Arc<TokioMutex<SemanticIndex>>` (realm/mod.rs:76)
- tokio Mutex serializes all access — invariant upheld

**Finding:** The `unsafe impl Sync` is sound given current usage. The risk is that a future
caller bypasses the Mutex wrapper. The safety comment documents this but there's no compile-time
enforcement.

## Decision

**Option 1: Accept current state with corrected documentation.**

The `unsafe impl Sync` is genuinely sound at the type level — the `&self`/`&mut self` split
IS the compile-time enforcement. A newtype wrapper (Option 2) is unnecessary because:
- `&self` methods are read-only at the FFI boundary (`*const EmbeddingIndex` on Zig side)
- `&mut self` methods require exclusive access (Rust borrow checker enforces this)
- `handle` field is private — no path to call mutating FFI from shared reference
- The TokioMutex is defense-in-depth, not a soundness requirement

The fix is documentation-only: rewrite the safety comment to state the actual soundness
argument instead of incorrectly claiming external locking dependence.

## Implementation

### Step 1: Rewrite the `unsafe impl Sync` safety comment
File: `markymark-kernels/src/embed.rs`, lines 91–101

Replace the current comment (including WARNING) with an accurate soundness argument:
- `&self` methods (`search`, `count`, `dimensions`) are read-only at FFI boundary
- `&mut self` methods (`add`, `remove`) require exclusive access via borrow checker
- `handle` is private — no mutation path from `&self`
- Note TokioMutex as defense-in-depth, not soundness requirement
- Keep `nosemgrep` annotation

TDD escape hatch: documentation-only change, no behavioral change.

### Step 2: Verify compilation
Run: `cargo check -p markymark-kernels`
Expected: clean compilation

### Step 3: Verify existing tests pass
Run: `cargo nextest run -p markymark-kernels`
Expected: all pass including `test_embedding_index_is_send_and_sync`

### Step 4: Commit and push
Message: `fix(kernels): correct unsafe impl Sync safety comment for EmbeddingIndex`

## Success Criteria

- [ ] Invariant verified or compile-time enforcement added
- [ ] Safety comment accurately describes soundness argument
- [ ] No incorrect claims about external locking being required

## Log

- [2026-03-23T14:15:19Z] [Seth] Diagnosis complete. Root cause: the unsafe impl Sync is actually sound — the safety comment is overly conservative. The WARNING claiming 'only safe under external read-write locking' is incorrect. The type is independently safe because: (1) &self methods (search, count, dimensions) call *const EmbeddingIndex on the Zig side — truly read-only, (2) &mut self methods (add, remove) require exclusive access enforced by Rust's borrow checker, (3) handle field is private — no path to call mutating FFI from &self, (4) the TokioMutex in SemanticIndex/RealmIndex is defense-in-depth, not a soundness requirement. Fix: update the safety comment to remove the incorrect WARNING about external locking being required for soundness. The Mutex is good practice but the type contract is sound independently.
- [2026-03-23T14:16:09Z] [Seth] Fix plan written. 4-step implementation: rewrite safety comment → cargo check → cargo nextest → commit+push. Documentation-only change, TDD escape hatch applies.
