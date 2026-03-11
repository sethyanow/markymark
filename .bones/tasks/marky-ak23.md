---
id: marky-ak23
title: 'Rust engine.rs corrections: fix Sync doc comment and neutral create-failure error code'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Two Copilot PR #40 findings in markymark-kernels/src/engine.rs. C2: Doc says Send+Sync but only Send implemented. C3: null from create mapped to -3 (OOM) but could be parse failure.

## Design

## Goal
Fix two issues in markymark-kernels/src/engine.rs found by Copilot on PR #40. One is a misleading doc comment, the other is a misleading error code.

## Effort Estimate
30-60 minutes

## Fix 1 (P3): Doc comment says Send+Sync but only Send implemented (Copilot C2)

**Problem**: Lines 70-73 say "implements Send and Sync via unsafe impls" but lines 83-90 show only Send is implemented, with a detailed SAFETY comment explaining why Sync is deliberately NOT implemented (get_blob mutates Zig-side cached_blob).

**Fix**: Replace lines 68-73 with:
\`\`\`rust
/// # Thread Safety
///
/// [\`DocumentEngine\`] implements \`Send\` via an unsafe impl; it intentionally
/// does **not** implement \`Sync\`. The underlying Zig heap allocation has no
/// thread-local state, so transferring ownership of a \`DocumentEngine\`
/// between threads is safe, but sharing \`&DocumentEngine\` across threads is
/// not. For concurrent use, wrap the engine in synchronization primitives
/// such as \`Arc<RwLock<DocumentEngine>>\` and share that wrapper instead.
\`\`\`

## Fix 2 (P4): null from create mapped to -3 (Copilot C3)

**Problem**: Line 112 maps null from marky_engine_create to KernelError::InternalError(-3). The -3 code means OOM in the Zig API, but create can fail for multiple reasons (invalid input, OOM, parse failure). Using -3 makes parse failures look like OOM in diagnostics.

**Fix**: Change to a neutral error code:
\`\`\`rust
return Err(KernelError::InternalError(0));
\`\`\`
And add a comment explaining why:
\`\`\`rust
// marky_engine_create returns null for any failure (invalid input,
// OOM, or parse error) without a specific error code. Use 0 as a
// neutral code rather than overloading -3 (OOM).
\`\`\`

## Success Criteria
- [ ] Doc comment accurately states Send-only, not Send+Sync
- [ ] create failure uses neutral error code (0), not -3
- [ ] cargo check -p markymark-kernels passes
- [ ] cargo test -p markymark-kernels passes
- [ ] cargo clippy -p markymark-kernels passes

## Implementation Checklist
- [ ] Fix 1: Update doc comment on DocumentEngine struct (lines 68-73)
- [ ] Fix 2: Change InternalError(-3) to InternalError(0) on line 112, add explanatory comment
- [ ] Run: cargo check -p markymark-kernels
- [ ] Run: cargo test -p markymark-kernels
- [ ] Commit with message referencing marky-ak23

## Anti-patterns
- Do NOT add a new KernelError variant just for create failure — the existing InternalError(i32) is fine
- Do NOT change the update() error mapping — those get specific error codes from the Zig API
