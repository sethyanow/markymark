---
id: marky-8jx
title: Implement lefthook pre-commit security hooks (clippy/fmt/gitleaks/cargo-audit)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ops]
parent: marky-84l
---




Configure lefthook pre-commit hooks for fmt/clippy/cargo-audit/gitleaks with verified fail behavior and no CI workflow regressions.

## Design

## Goal
Implement the pre-commit security layer of epic marky-84l using lefthook with no Python runtime dependency.

## Effort Estimate
4-6 hours

## Context
- Completed marky-ops: security.yml added with advisory-only cargo-audit/cargo-deny/CodeQL/Semgrep jobs.
- Remaining epic criteria include local developer guardrails via lefthook.
- Existing ci.yml must remain unchanged.

## Implementation
1. Add `lefthook` setup for contributors:
   - Add installation instructions in contributor-facing docs.
   - Ensure commands are reproducible on macOS/Linux shells.
2. Create `lefthook.yml` in repo root with `pre-commit` hooks:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo audit`
   - `gitleaks protect --staged --redact`
3. Configure execution semantics:
   - deterministic hook order
   - fail-fast behavior
   - no shell wrappers that mask exit codes.
4. Validate local execution paths:
   - run `lefthook install`
   - run `lefthook run pre-commit` on clean tree
   - run a negative test (temporary staged secret sample) to confirm gitleaks blocks commit, then remove sample.
5. Confirm no regressions:
   - verify `.github/workflows/ci.yml` unchanged
   - run `cargo test --workspace`

## Success Criteria
- [ ] `lefthook.yml` exists and defines `pre-commit` hooks for fmt, clippy, cargo-audit, and gitleaks.
- [ ] `lefthook run pre-commit` succeeds on a clean working tree.
- [ ] gitleaks hook fails on staged secret-like content and passes after sample removal.
- [ ] `cargo test --workspace` passes after hook integration.
- [ ] `.github/workflows/ci.yml` remains unchanged.
- [ ] Hook config contains no placeholder text, TODOs, or disabled checks.

## Anti-Patterns
- Do not introduce Python pre-commit framework/runtime dependencies.
- Do not weaken clippy/fmt strictness to make hooks pass.
- Do not skip gitleaks negative-test verification.
- Do not suppress cargo-audit failures in hook commands.

## Key Considerations (SRE refinement)
- Tool availability: if `gitleaks` or `cargo-audit` is missing, fail with explicit install hints.
- False positives: allowlist only when justified and reviewed; no blanket ignores.
- Cross-platform behavior: avoid shell assumptions that break on default shells.
- Performance: avoid expensive redundant invocations, but never skip required security checks.
- Failure semantics: verify each hook command returns non-zero on failures and blocks commit.

## Verification
- `lefthook install`
- `lefthook run pre-commit`
- `cargo test --workspace`
- `git diff -- .github/workflows/ci.yml`
