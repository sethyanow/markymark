---
id: marky-ozi
title: 'Phase 4.7: Audit remaining epic criteria and close marky-0xtn'
status: open
type: task
priority: 2
parent: marky-0xtn
---


## Context

- Phases 4.1-4.6 completed all deletion work for the marky-0xtn epic.
- 8/14 epic criteria are checked. 6 remain unchecked:
  - Criterion 1: CEngineResult struct matches DocumentEngine state exactly (13 types + 3 metadata)
  - Criterion 2: Parity tests prove from_engine_result == from_blob for diverse inputs
  - Criterion 11: All tests passing (Zig + Rust 1123+)
  - Criterion 12: Pre-commit hooks passing
  - Criterion 13: generation field present in CEngineResult (u64, monotonic)
  - Criterion 14: _reserved[32] bytes in CEngineResult for future incremental fields
- Criteria 1, 2, 13, 14 were built during Phase 1 (marky-e0kp, commit `2cd1310`). They likely still hold but need re-verification after 6 phases of changes.
- Criterion 11 (tests): 983/983 Rust tests pass as of Phase 4.6 (`ab8c531`). Zig tests also pass.
- Criterion 12 (hooks): not yet verified in this epic branch.
- This is the final gate before closing the epic. If all criteria verify, close the epic. If any fail, create targeted fix tasks.

## Requirements

- Verify all 6 unchecked epic criteria against current code state
- Check off verified criteria in the epic skeleton
- If all 14 criteria satisfied: prepare epic for closure
- If any criterion fails: create targeted fix task, do NOT close epic

## Implementation

1. **Verify criterion 1** (CEngineResult coverage) — Use LSP on `zig/src/engine/ffi_types.zig` to confirm CEngineResult has all 13 element types + metadata fields. Cross-reference with `markymark-kernels/src/engine_ffi.rs` Rust mirrors.

2. **Verify criterion 2** (parity tests) — Find parity tests that compare from_engine_result output with known-good baselines. Run them. Prior from_blob function no longer exists (deleted in Phase 4.5), so parity is measured against test fixtures, not live from_blob calls.

3. **Verify criterion 11** (all tests passing) — Run `cargo nextest --workspace` + `zig build test`. Report exact counts.

4. **Verify criterion 12** (pre-commit hooks) — Run pre-commit hooks or simulate a commit check. Verify all hooks pass cleanly.

5. **Verify criterion 13** (generation field) — LSP hover on CEngineResult in both Zig and Rust to confirm `generation: u64` field exists and is monotonic (increments on each engine update).

6. **Verify criterion 14** (_reserved bytes) — LSP hover to confirm `_reserved: [u8; 32]` or equivalent padding in CEngineResult.

7. **Update epic criteria** — Check off all verified criteria in `.bones/tasks/marky-0xtn.md`.

8. **If all pass** — Invoke `litepowers:review-implementation` for final validation, then close epic.

## Success Criteria

- [ ] Criterion 1 verified: CEngineResult has 13 element types + metadata
- [ ] Criterion 2 verified: parity tests exist and pass
- [ ] Criterion 11 verified: all tests pass (report exact count)
- [ ] Criterion 12 verified: pre-commit hooks pass
- [ ] Criterion 13 verified: generation field present (u64, monotonic)
- [ ] Criterion 14 verified: _reserved[32] bytes present
- [ ] All 14 epic criteria checked in marky-0xtn.md
- [ ] Epic closed (or targeted fix tasks created for failures)

## Anti-Patterns

- FORBIDDEN: checking off criteria without running verification commands
- FORBIDDEN: closing the epic if ANY criterion fails verification
- FORBIDDEN: modifying production code to make criteria pass — this is audit only, create fix tasks if needed
