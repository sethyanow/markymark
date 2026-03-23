---
id: marky-aemm
title: 'Debounce race: stale batch applied after close/reopen (residual from marky-0mr.1)'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Fix a residual race condition in the debounce logic (server.rs:198-239) where a stale
change batch can overwrite fresh content after a close/reopen sequence. This was NOT
fully addressed by marky-0mr.1 (commit 7711f33) which added debounce cancellation on
did_close — the race occurs when the debounce task wakes and drains pending_changes
BEFORE did_close can abort it.

## Root Cause

The debounce task (lines 204-211) atomically removes its abort handle AND drains
pending_changes under the DebounceState mutex. After releasing that mutex, it awaits
the state write lock (line 220) to apply changes.

If did_close arrives in this gap:
1. Debounce task wakes from sleep, locks DebounceState mutex
2. Removes its own abort handle (line 206) — no longer cancellable
3. Drains pending_changes (line 207) — local variable now holds stale batches
4. Releases mutex (line 211)
5. —— RACE WINDOW OPENS ——
6. did_close locks DebounceState mutex (line 247)
7. Tries to remove abort handle — GONE (step 2)
8. Tries to remove pending_changes — GONE (step 3)
9. did_close proceeds: closes document in state (line 255)
10. did_open arrives: opens document with fresh content (line 148)
11. Debounce task finally acquires state write lock (line 220)
12. Applies stale batches to the reopened document — CORRUPTS FRESH CONTENT

Note: apply_document_changes (state/mod.rs:257) has a documents.get_mut() guard that
returns early if the URI is absent — so close-without-reopen is safe. The dangerous
case is specifically close→reopen, where the document IS back in state.

## Effort Estimate

2-4 hours (code change is small; test is the bulk of the work)

## Fix: Document Generation Counter

Add a monotonic generation counter to DebounceState, incremented on did_open and did_close.
The debounce task captures the generation when it drains batches, then checks it again
after acquiring the state write lock. If generation changed, discard the batch.

### Implementation Checklist

- [ ] Add `document_generations: HashMap<DocumentUri, u64>` field to `DebounceState` (server.rs:22-30)
- [ ] In `did_open` (server.rs:143-152): after acquiring state write lock and calling
      `open_document`, lock DebounceState and increment (or insert) the generation for this URI
- [ ] In `did_close` (server.rs:242-262): inside the existing DebounceState lock scope
      (line 247), increment the generation for this URI (do NOT remove — the generation must
      survive close so post-close debounce tasks see the bump)
- [ ] In the debounce task (server.rs:204-211): when draining batches under the mutex,
      also read `document_generations.get(&uri).copied().unwrap_or(0)` and store as
      `captured_gen: u64`
- [ ] In the debounce task (server.rs:219-225): after acquiring `state.write().await`,
      lock DebounceState again, read current generation, compare with `captured_gen`.
      If different, drop batches and return without applying
- [ ] Write regression test (see Test Plan below)
- [ ] Verify all existing debounce tests still pass

### Alternative Considered: Check document existence

Checking `state_w.documents.contains_key(uri)` before applying is insufficient because
the close→reopen sequence means the document IS present — just with different content.
A generation counter is the minimal correct fix.

## Success Criteria

- [ ] New regression test `test_close_reopen_during_debounce_drain_window` passes —
      verifies that stale batch is discarded when document is closed and reopened
      after debounce task drains but before it acquires state lock
- [ ] All 5 existing debounce tests pass unchanged
- [ ] Full `cargo nextest` passes
- [ ] `cargo clippy --workspace --all-targets` passes with zero warnings
- [ ] Generation counter correctly incremented on both open and close paths

## Test Plan

### Regression test: test_close_reopen_during_debounce_drain_window

This test must exercise the exact race window. Since timing-based reproduction is
fragile, use a deterministic approach:

**Option A (Recommended): Expose generation check via test helper**
- Add a `#[cfg(test)]` method on Backend that directly exercises the "drain then
  check generation" path without relying on tokio sleep timing
- Open document, fire change, wait for debounce to fire normally, verify it works
- Then: open document, fire change, manually simulate: drain batches + bump generation
  + attempt apply — verify apply is skipped

**Option B: Timing-based with barrier**
- Use a `tokio::sync::Notify` or `tokio::sync::Barrier` injected via test to
  pause the debounce task after draining but before acquiring state lock
- While paused: call did_close + did_open
- Release barrier — debounce task should detect generation mismatch and skip

**Option A is preferred** because it doesn't depend on timing and is deterministic.

### What specific bugs each test catches:

| Test | Bug it catches |
|------|---------------|
| test_close_reopen_during_debounce_drain_window | Stale batch overwrites fresh content after close/reopen |
| test_debounce_defers_reparse_until_pause (existing) | Debounce fires too early |
| test_did_close_cancels_pending_debounce (existing) | Debounce not cancelled on close |
| test_close_during_debounce_index_unchanged (existing) | Close-only leaves stale index |
| test_empty_change_batch_is_noop (existing) | Empty batch triggers debounce |
| test_single_change_applies_after_debounce (existing) | Normal debounce path broken |

## Edge Cases

**EC-1: Rapid close→open→close→open (multiple cycles)**
Generation counter increments on each open and close. Even with multiple cycles,
the captured generation will differ from current, and stale batches are discarded.

**EC-2: Multiple URIs — generation isolation**
Each URI has its own generation counter in the HashMap. A close/reopen on URI-A
must NOT affect debounce for URI-B. Verify this in tests if time permits.

**EC-3: Generation counter overflow (u64)**
At 1 billion operations/second, u64 overflow takes ~584 years. Not a concern.
Do NOT use wrapping arithmetic — straight increment is fine.

**EC-4: did_close without prior did_open**
Generation map may not have an entry. Use `unwrap_or(0)` when reading. Incrementing
a missing entry should insert 1 (use `entry().and_modify(|g| *g += 1).or_insert(1)`).

**EC-5: Debounce task for URI that was never opened**
The `apply_document_changes` guard at state/mod.rs:257 (`documents.get_mut()`) already
handles this — returns early. Generation check adds a second layer but the existing
guard is the primary defense for this case.

**EC-6: Memory leak from generation map**
Entries accumulate for every URI ever opened. For an LSP server this is bounded by
the number of files in the workspace (typically <10,000). Not a concern for v1.
If needed later, prune entries in did_close after a delay.

## Anti-patterns

- Do NOT use `unwrap()` or `expect()` on the generation lookup — use `unwrap_or(0)`
- Do NOT remove generation entries in did_close — the debounce task needs to see the bump
- Do NOT use `AtomicU64` instead of the mutex — the generation must be read atomically
  WITH the pending_changes drain (both under DebounceState mutex)
- Do NOT add a separate mutex for generations — use the existing DebounceState mutex
- Do NOT make the test timing-dependent — use deterministic approach (Option A)

## Key Considerations

**Why existing test passes despite the bug:**
test_did_close_cancels_pending_debounce works because did_close arrives while the
debounce task is still sleeping (within the 75ms window). The abort handle IS still
present, so cancellation succeeds. The bug only manifests when the debounce task
wakes up and drains BEFORE did_close arrives — a narrow timing window that the
current test never hits.

**Scope containment:**
All changes are in server.rs (DebounceState struct + 3 methods). No changes needed
to state/mod.rs — apply_document_changes keeps its existing guard unchanged. This is
purely a debounce-layer fix.

**Performance impact:**
One HashMap lookup + one u64 comparison per debounce apply. Negligible.

## References

- Prior fix: marky-0mr.1 (commit 7711f33) — added did_close cancellation
- Code: markymark-lsp/src/server.rs lines 22-262
- State: markymark-lsp/src/state/mod.rs lines 250-329
- Tests: markymark-lsp/tests/debounce.rs
