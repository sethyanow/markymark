---
id: marky-c44x
title: 'PR#41 perf: Flatten debounce batches to single apply_document_changes call'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Flatten multiple debounce batches into a single apply_document_changes call to
eliminate redundant full reparses (Zig FFI + realm re-index) during the debounce
flush path.

## Root Cause

In markymark-lsp/src/server.rs:274-276, the debounce flush loop calls
apply_document_changes() per buffered batch. Each call triggers Phase 2:
realm.remove_document + build_markdown_index_via_engine (Zig FFI) +
realm.add_document. With 3-5 batches from fast typing, this means 3-5 full
reparses instead of 1.

apply_document_changes (state/mod.rs:250) already processes changes sequentially
within a batch (Phase 1). Flattening preserves ordering and is semantically
equivalent for the text-edit phase, eliminating N-1 redundant Phase 2 calls.

## Effort Estimate

1-2 hours

## Success Criteria

- [ ] Debounce flush calls apply_document_changes exactly once (not N times)
- [ ] Changes are applied in original order (batch1_changes ++ batch2_changes ++ ...)
- [ ] Test helper try_apply_drained (line 924) is updated to match
- [ ] All existing debounce tests pass (markymark-lsp/tests/debounce.rs)
- [ ] cargo test -p markymark-lsp passes
- [ ] cargo clippy --workspace --all-targets clean

## Implementation Checklist

- [ ] In server.rs debounce task (lines 274-276): replace for-loop with flatten:
      let all_changes: Vec<crate::state::DocumentChange> = batches.into_iter().flatten().collect();
      state_w.apply_document_changes(&doc_uri_clone, all_changes);
- [ ] In try_apply_drained (lines 924-927): same flatten pattern
- [ ] Run debounce integration tests: cargo test -p markymark-lsp
- [ ] Verify with a quick manual test or trace that only one engine reparse occurs per flush

## Key Considerations (SRE Review)

**Safety: Incremental changes are order-dependent**
Each DocumentChange::Incremental has byte offsets relative to text after previous
changes were applied. Flattening preserves this ordering because batches are
collected in temporal order and each batch's changes are already sequential.
into_iter().flatten() produces: [batch1[0], batch1[1], ..., batch2[0], batch2[1], ...].
This is identical to applying them one batch at a time.

**Risk: Tests relying on intermediate re-index state**
If any test checks document index state between batch applications, flattening would
break it. Review debounce.rs tests for such patterns. The current tests use
drain_pending + try_apply_drained, which mirrors the real implementation.

**Performance impact estimate**
At 50KB doc size, each Zig reparse is ~4-5ms. With 3 batches, savings = ~8-10ms per
debounce window. Small but meaningful for responsive editing.

## Anti-patterns

- Do NOT change the change application ordering (must be temporal)
- Do NOT skip the generation check (it must remain between drain and apply)
- Do NOT change apply_document_changes internals (change is at call site only)
