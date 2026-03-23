---
id: marky-ops
title: Implement security.yml with advisory-only jobs and SARIF uploads
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-yjg]
parent: marky-84l
---




Create a new GitHub Actions workflow at .github/workflows/security.yml implementing cargo-audit, cargo-deny, CodeQL, and Semgrep in parallel with advisory-only behavior, while keeping existing ci.yml unchanged.

## Design

## Goal
Implement the CI security workflow slice of epic marky-84l.

## Effort Estimate
6-8 hours

## Context
- Completed marky-yjg baseline scan and deny.toml configuration.
- Must add a separate workflow file; ci.yml must remain untouched.
- Epic requires advisory-only scanning (continue-on-error true on each job).

## Implementation
1. Create .github/workflows/security.yml with triggers:
   - push branches: main, feat/**
   - pull_request branches: main
2. Define four parallel jobs with clear names:
   - cargo-audit
   - cargo-deny
   - codeql
   - semgrep
3. Add common job hardening:
   - permissions minimized per job
   - checkout at start
   - Rust toolchain setup where needed
   - continue-on-error: true on each security job
4. Implement cargo-audit job:
   - install cargo-audit (locked)
   - run cargo audit
5. Implement cargo-deny job:
   - install cargo-deny (locked)
   - run cargo deny check against repo deny.toml
6. Implement CodeQL job:
   - github/codeql-action/init for rust
   - autobuild step
   - github/codeql-action/analyze with category for workflow
   - confirm SARIF upload path via analyze action
7. Implement Semgrep job:
   - run semgrep with p/rust plus local custom .semgrep rules directory if present
   - output SARIF file
   - upload SARIF to Security tab via github/codeql-action/upload-sarif
8. Validate behavior and regressions:
   - verify ci.yml unchanged
   - run cargo test --workspace
   - run a workflow lint/sanity check if available

## Success Criteria
- [ ] .github/workflows/security.yml exists and parses as valid GitHub Actions YAML.
- [ ] Workflow triggers exactly on push (main, feat/**) and pull_request (main).
- [ ] Four jobs exist and run independently: cargo-audit, cargo-deny, codeql, semgrep.
- [ ] All security jobs are advisory-only with continue-on-error: true.
- [ ] CodeQL analyze uploads SARIF to GitHub Security.
- [ ] Semgrep job uploads SARIF to GitHub Security.
- [ ] ci.yml is byte-for-byte unchanged in this task.
- [ ] cargo test --workspace passes after workflow changes.

## Anti-Patterns
- Do not edit existing .github/workflows/ci.yml.
- Do not make security jobs blocking.
- Do not split tools across multiple workflow files.
- Do not add Python-based pre-commit tooling in this task.
- Do not suppress scan findings in workflow commands.

## Key Considerations (SRE Review)
- Trigger drift: ensure feat/** is branch filter, not path filter.
- Fork PR permissions: use least-privilege permissions and avoid write scopes except where SARIF upload needs security-events: write.
- Tool/network failures: continue-on-error preserves advisory semantics but still surfaces output.
- Semgrep noise: include test-file exclusions only if required by epic anti-patterns; avoid over-excluding source files.
- SARIF artifact naming: unique filenames per tool to avoid upload collision.
- Reproducibility: pin actions to stable major versions and use locked install flags where available.

## Verification
- cargo test --workspace
- git diff -- .github/workflows/ci.yml (must be empty)
- manual inspection of .github/workflows/security.yml for 4 jobs + continue-on-error + SARIF upload steps
