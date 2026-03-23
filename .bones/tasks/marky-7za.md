---
id: marky-7za
title: Implement custom Semgrep rules + tests for unsafe/concurrency/input validation
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8jx]
parent: marky-84l
---



## Design

## Goal
Implement and validate custom Semgrep rules required by epic marky-84l for unsafe patterns, async concurrency hazards, and MCP/LSP input validation.

## Context
- Completed: marky-yjg (baseline + deny.toml), marky-ops (security.yml), marky-8jx (lefthook pre-commit hooks).
- Remaining epic criteria are centered on `.semgrep/` custom coverage and proving those rules are useful and low-noise.
- Epic anti-patterns forbid noisy rules on test files and forbid shipping custom rules without test cases.

## Implementation
1. Create `.semgrep/unsafe-patterns.yml` with focused Rust rules for unsafe misuse patterns relevant to this repo.
2. Create `.semgrep/concurrency.yml` with rules targeting `.await` while holding locks and similar async contention hazards.
3. Create `.semgrep/input-validation.yml` with rules for MCP/LSP boundary validation (unchecked user-controlled input paths/uris/options).
4. Add Semgrep rule tests/fixtures so each rule has at least one positive and one negative case.
5. Ensure rule scope excludes test fixtures and test directories when needed to avoid anti-pattern noise.
6. Run semgrep locally against repo and fixtures to validate expected hits only.
7. Verify `.github/workflows/security.yml` already consumes `.semgrep/**` (no ci.yml modifications).
8. Run `cargo test --workspace` regression check.

## Success Criteria
- [ ] `.semgrep/unsafe-patterns.yml` exists with at least one high-signal rule.
- [ ] `.semgrep/concurrency.yml` exists with at least one `.await`/lock hazard rule.
- [ ] `.semgrep/input-validation.yml` exists with at least one boundary validation rule.
- [ ] Each custom rule has explicit test coverage (positive + negative examples).
- [ ] Local Semgrep run shows expected findings from fixtures and no broad test-file noise.
- [ ] `cargo test --workspace` passes.
- [ ] `.github/workflows/ci.yml` remains unchanged.

## SRE Corner-Case Refinement
- Rule precision: guard against regex-only overmatching; prefer AST/pattern constraints where possible.
- Noise control: exclude test directories and generated files explicitly.
- Drift resistance: include comments in each rule on intent and false-positive boundaries.
- Runtime constraints: keep scans bounded to avoid local/CI timeout blowups.
- Verification artifacts: capture semgrep command output summary in task notes.

## Verification
- semgrep scan --config .semgrep .
- cargo test --workspace
- git diff -- .github/workflows/ci.yml
