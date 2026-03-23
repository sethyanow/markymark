---
id: marky-4atp
title: 'PR#41 quality: Test lengths, glob import, DRY scanner, eprintln→tracing'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Four code quality improvements identified in PR#41 review, grouped by low risk.

## Items (prioritized within track)

### E1: Hard-coded test lengths in exports.zig (RECOMMENDED)
File: zig/src/engine/exports.zig tests (lines 112-256)
Issue: Tests pass literal byte counts (9, 6, 13) instead of .len.
Fix: Use text.ptr + @intCast(text.len) pattern. Keep null+0 cases unchanged.
Value: Prevents sentinel off-by-one if test strings are edited.

### E2: eprintln! → tracing::warn! in state/mod.rs (RECOMMENDED)
File: markymark-lsp/src/state/mod.rs:282,295
Issue: Two eprintln! calls are not filterable. tracing is already a dependency.
Fix: Replace with tracing::warn!(target = "markymark_lsp", ...).
Value: Filterable structured logs in production.

### E3: from_blob.rs glob import → explicit imports (LOW VALUE)
File: markymark-index/src/document/from_blob.rs:28
Issue: use markymark_core::prelude::* hides used types.
Fix: Enumerate explicit imports until cargo check passes.
Value: Readability only. Low priority.

### E4: scanner.rs DRY helpers (LOW VALUE, RISKY)
File: markymark-core/src/scanner.rs:222-331
Issue: Three methods duplicate extraction→result mapping.
Fix: Extract shared mapping helpers.
Value: Reduces ~30 lines of duplication.
Risk: Changes public API boundary — requires more careful testing.
Recommend: DEFER. Not worth the churn for 30 lines in a working module.

## Effort Estimate

2 hours for E1+E2. E3 is 30 min. E4 is deferred.

## Success Criteria

- [ ] exports.zig tests use .len instead of literal counts (E1)
- [ ] state/mod.rs uses tracing::warn! instead of eprintln! (E2)
- [ ] zig build test passes (E1)
- [ ] cargo test -p markymark-lsp passes (E2)
- [ ] cargo clippy --workspace --all-targets clean

## Anti-patterns

- Do NOT refactor scanner.rs without comprehensive test coverage (E4 risk)
- Do NOT change the null+0 test cases in exports.zig (those test null handling, not length)
