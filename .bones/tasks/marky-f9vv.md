---
id: marky-f9vv
title: Fix UB in DocumentIndex::arena_ref mutex escape
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

## Problem

`DocumentIndex::arena_ref` at `markymark-index/src/document/mod.rs:83-98` escapes the MutexGuard lifetime by extracting a raw pointer, dropping the guard, then dereferencing. This violates Rust aliasing rules because `Bump` uses interior mutability (`Cell`) internally — generating a `&Bump` outside the guard is UB even though the access pattern is single-threaded.

```rust
let arena_guard = owner.arena.lock().unwrap();
let arena_ptr: *const DocumentArena = &*arena_guard as *const DocumentArena;
drop(arena_guard);
unsafe { (*arena_ptr).bump() }  // UB: &Bump from UnsafeCell without guard
```

## Context

- `arena_ref` is only called during `from_ast` construction, never concurrently
- The Mutex exists to satisfy `self_cell`'s API, not for actual synchronization
- Miri would flag this

## Fix Direction

Either:
1. Use `self_cell`'s builder API to pass the arena directly without needing a Mutex
2. Replace Mutex with a private `UnsafeCell` + documented safety proof that construction is single-threaded and the arena is immutable after init

Do NOT use `UnsafeCell<DocumentArena>` with blanket `unsafe impl Send + Sync` — that's swapping one unsoundness for another.

## Discovery

Identified via external code review (Gemini 3.1 Pro). Confirmed by manual inspection.

## Files

- `markymark-index/src/document/mod.rs:83-98`

## Design

## Problem

\`DocumentIndex::arena_ref\` at \`markymark-index/src/document/mod.rs:91-107\` escapes the MutexGuard lifetime by extracting a raw pointer, dropping the guard, then dereferencing. This violates Rust aliasing rules because \`Bump\` uses interior mutability (\`Cell\`) internally — generating a \`&Bump\` outside the guard is UB even though the access pattern is single-threaded.

```rust
let arena_guard = owner.arena.lock().unwrap();
let arena_ptr: *const DocumentArena = &*arena_guard as *const DocumentArena;
drop(arena_guard);
unsafe { (*arena_ptr).bump() }  // UB: &Bump from UnsafeCell without guard
```

## Root Cause

The Mutex exists solely to satisfy Send+Sync requirements for tower-lsp (\`RwLock<ServerState>\` requires \`ServerState: Send + Sync\`). It is NOT needed for actual synchronization. The \`self_cell\` builder closure receives \`&DocumentOwner\`, and \`MutexGuard\`'s borrow lifetime is local to the closure — it cannot provide \`&Bump\` with the owner's lifetime. The raw-pointer workaround was the original hack to bridge this gap.

## Fix Design (CHOSEN: Option 1 — remove Mutex, add unsafe impl Sync)

### Changes

1. **Remove Mutex wrapper**: Change \`arena: Mutex<DocumentArena>\` to \`arena: DocumentArena\`
2. **Delete \`arena_ref()\` method entirely**: No longer needed
3. **Update 3 builder closures**: Replace \`Self::arena_ref(owner)\` / \`DocumentIndex::arena_ref(owner)\` with \`owner.arena.bump()\`
   - \`from_ast_with_overrides_opt\` (mod.rs:377)
   - \`from_scan\` (mod.rs:576)
   - \`from_blob\` (from_blob.rs:499)
4. **Update 3 \`DocumentOwner\` construction sites**: Remove \`Mutex::new()\` wrapper
   - \`from_ast_with_overrides_opt\` (mod.rs:373-374)
   - \`from_scan\` (mod.rs:572-573)
   - \`from_blob\` (from_blob.rs:495-496)
5. **Add \`unsafe impl Sync for DocumentIndex\`** with safety proof
6. **Remove \`use std::sync::Mutex\`**
7. **Update doc comment** on \`DocumentIndex\` (lines 72-84)

### Safety Proof for \`unsafe impl Sync\`

\`DocumentArena\` wraps \`bumpalo::Bump\` which is \`Send + !Sync\`. \`Bump\` is \`!Sync\` because its internal allocation pointer uses \`Cell\` (interior mutability). However:

1. **Construction-only mutation**: \`Bump::alloc()\` (interior mutation via Cell) is ONLY called inside the \`self_cell\` builder closure during \`DocumentIndexCell::new()\`. This closure runs synchronously, single-threaded.
2. **Post-construction freeze**: After \`new()\` returns, no code path reaches \`bump.alloc()\`. All public accessors use \`borrow_dependent()\` which returns immutable references to arena-backed slices.
3. **No public arena access**: \`DocumentOwner\` is private. \`cell\` field is private. No public API exposes \`&Bump\` or \`&DocumentArena\`.
4. **No \`borrow_owner()\` calls**: Confirmed via grep — nobody calls \`borrow_owner()\` or \`with_dependent()\` on the cell outside of construction.

Therefore sharing \`&DocumentIndex\` across threads is safe: all shared access is read-only.

## Success Criteria

- [ ] \`arena_ref()\` method and its unsafe raw-pointer dereference are deleted
- [ ] \`Mutex<DocumentArena>\` replaced with bare \`DocumentArena\` in \`DocumentOwner\`
- [ ] 3 builder closures use \`owner.arena.bump()\` directly
- [ ] \`unsafe impl Sync for DocumentIndex {}\` present with safety proof comment
- [ ] Compile-time test: \`fn assert_send_sync<T: Send + Sync>() {}\` passes for DocumentIndex
- [ ] All existing tests pass (cargo nextest -p markymark-index)
- [ ] All alignment tests pass (cargo nextest -p markymark-cli)
- [ ] Clippy clean with zero warnings
- [ ] No \`std::sync::Mutex\` import remaining in document/mod.rs

## Edge Cases (SRE Review)

**Thread safety after fix**: The \`unsafe impl Sync\` is sound ONLY if no future code adds post-construction mutation. Mitigations:
- Keep \`DocumentOwner\` private (prevents external access to arena)
- Add compile-time Send+Sync assertion test (catches regressions)
- Doc comment on struct warns about the invariant

**self_cell Send/Sync derivation**: self_cell auto-derives Send/Sync based on owner/dependent. With bare \`DocumentArena\` (\`!Sync\`), the cell becomes \`!Sync\`. Our \`unsafe impl Sync for DocumentIndex\` overrides this at the wrapper level. This is the correct granularity — the invariant is about DocumentIndex as a whole.

**Regression risk**: Zero behavioral change. Only the Send/Sync mechanism changes. All allocation patterns identical.

## Anti-patterns
- ❌ Do NOT use \`UnsafeCell<DocumentArena>\` with blanket \`unsafe impl Send + Sync\` — trades one unsoundness for another
- ❌ Do NOT add \`unsafe impl Sync for DocumentOwner\` — too broad, should be at DocumentIndex level
- ❌ Do NOT use \`parking_lot::Mutex\` — same lifetime escape problem as std::sync::Mutex
- ❌ Do NOT call \`borrow_owner()\` on the cell outside construction code

## Files
- \`markymark-index/src/document/mod.rs\` (main changes)
- \`markymark-index/src/document/from_blob.rs\` (builder closure update)
- \`markymark-index/src/document/tests.rs\` (add Send+Sync assertion test)
