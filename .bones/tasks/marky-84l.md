---
id: marky-84l
title: '[EPIC] SAST Security Scanning: CI, Pre-commit, Custom Rules, Baseline'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
---





## Requirements (IMMUTABLE)
- CI workflow (security.yml) with 4 parallel jobs: cargo-audit, cargo-deny, CodeQL, Semgrep
- All CI security checks are advisory-only (do not block PR merge)
- SARIF upload for CodeQL and Semgrep results to GitHub Security tab
- Pre-commit hooks via lefthook: clippy, fmt, gitleaks, cargo-audit
- Custom Semgrep rules for: unsafe patterns, concurrency issues, MCP input validation
- cargo-deny config (deny.toml) with license allowlist, advisory checks, source restrictions
- Baseline security scan run locally with findings documented

## Success Criteria (MUST ALL BE TRUE)
- [ ] security.yml workflow runs on push to main/feat/** and PRs to main
- [ ] cargo-audit job detects known dependency vulnerabilities
- [ ] cargo-deny job enforces license/advisory/source policy
- [ ] CodeQL job runs Rust security queries and uploads SARIF
- [ ] Semgrep job runs built-in + custom rules and uploads SARIF
- [ ] All 4 CI jobs are advisory-only (continue-on-error: true)
- [ ] lefthook.yml installed with clippy, fmt, gitleaks, cargo-audit hooks
- [ ] lefthook runs successfully on pre-commit
- [ ] Custom Semgrep rules exist in .semgrep/ directory
- [ ] Custom rules cover: unsafe patterns, concurrency (.await + locks), input validation
- [ ] deny.toml configured with license allowlist and advisory checks
- [ ] Baseline scan completed and findings documented
- [ ] All existing tests still pass (cargo test --workspace)
- [ ] Existing CI (ci.yml) unmodified and still passing

## Anti-Patterns (FORBIDDEN)
- ❌ NO blocking gates on security scans (requirement: advisory-only, user chose this explicitly)
- ❌ NO modifications to existing ci.yml (separation: security workflow is independent)
- ❌ NO Python dependencies for pre-commit (requirement: lefthook chosen specifically to avoid Python)
- ❌ NO Semgrep rules that fire on test code (noise: test assertions and mocks legitimately use patterns that look unsafe)
- ❌ NO cargo-deny license denials without allowlist review (false positives: many Rust crates use Unicode-3.0, ISC, etc.)
- ❌ NO custom rules without test cases (validation: untested rules produce false positives or miss real issues)
- ❌ NO disabling clippy warnings to satisfy security scans (regression: existing lint quality must be preserved)

## Approach
Single security.yml workflow with 4 parallel jobs. Pre-commit via lefthook (Go binary, no runtime deps). Custom Semgrep rules in .semgrep/ directory organized by category. cargo-deny via deny.toml at project root. Baseline scan run locally before CI activation.

## Architecture
- .github/workflows/security.yml — 4-job CI security workflow
- lefthook.yml — pre-commit hook config
- .semgrep/unsafe-patterns.yml — unsafe code rules
- .semgrep/concurrency.yml — async concurrency rules
- .semgrep/input-validation.yml — MCP/LSP input rules
- deny.toml — cargo-deny policy config

## Design Rationale
### Problem
Project has zero security scanning. No dependency auditing, no SAST, no secret detection, no pre-commit hooks. As a Rust LSP/MCP server processing untrusted markdown input, this is a gap.

### Research Findings
**Codebase:**
- .github/workflows/ci.yml — existing CI with clippy, fmt, test, alignment, benchmarks
- .github/workflows/release.yml — multi-platform release with git-cliff
- No security configs, no pre-commit hooks, no deny.toml, no SECURITY.md
- Clippy already runs with -D warnings in CI

**External:**
- CodeQL Rust support GA since 2.23.3 — 17 security queries covering injection, crypto, memory safety
- Semgrep Rust GA since v1.10.0 — 70+ rules, pattern-based
- cargo-audit 0.22.1 — RustSec advisory DB, actively maintained
- cargo-deny 0.19.0 — license/advisory/ban/source checks
- lefthook — Go-based pre-commit, single binary, no Python

### Approaches Considered

#### 1. Single security.yml workflow ✓
**What it is:** One new workflow file with 4 parallel jobs. Clean separation from build/test.
**Chosen because:** Single management point, parallel jobs keep runtime fast, doesn't bloat existing ci.yml.

#### 2. Integrated into existing ci.yml ❌
**Why explored:** Fewer files to manage.
**REJECTED BECAUSE:** Mixes build/test with security concerns. ci.yml already has 3 jobs. Would grow to 7 jobs in one file.
**DO NOT REVISIT UNLESS:** GitHub Actions adds workflow composition that makes multi-file management painful.

#### 3. Per-tool workflows ❌
**Why explored:** Fine-grained control per tool.
**REJECTED BECAUSE:** 3 new workflow files is scattered. Management overhead doesn't justify flexibility for a single-maintainer project.
**DO NOT REVISIT UNLESS:** Team grows and different people own different security tools.

### Scope Boundaries
**In scope:**
- CI security workflow (4 tools)
- Pre-commit hooks (lefthook)
- Custom Semgrep rules (3 categories)
- cargo-deny config
- Baseline scan + findings doc

**Out of scope:**
- SECURITY.md / vulnerability disclosure policy (separate concern)
- Dependabot config (separate concern)
- Code coverage tooling (separate concern)
- Miri/MIRAI (experimental, not CI-ready)
- cargo-geiger (useful but separate from SAST)
- SonarQube (overkill for this project)

### Open Questions
- What license exceptions will deny.toml need? (discover during baseline)
- Will CodeQL autobuild work with workspace, or need manual build step? (test during implementation)
- gitleaks may need custom allowlist for test fixtures (discover during baseline)

## Design Discovery (Reference Context)

### Key Decisions Made
| Question | User Answer | Implication |
|----------|-------------|-------------|
| Gate policy? | Advisory only | continue-on-error: true on all security jobs |
| CI tools? | Full stack | cargo-audit + cargo-deny + CodeQL + Semgrep |
| Pre-commit framework? | lefthook | lefthook.yml, Go binary, no Python |
| Custom rule focus? | All categories | unsafe + concurrency + input validation rules |

### Research Deep-Dives
#### Rust SAST Ecosystem 2026
**Question:** What tools are production-ready for Rust SAST?
**Sources:** Semgrep docs, CodeQL docs, cargo-audit/deny repos, Sherlock 2026 guide
**Findings:** CodeQL GA with 17 queries, Semgrep GA with 70+ rules, cargo-audit/deny stable
**Conclusion:** Full stack is viable — all tools are mature

#### Pre-commit Options
**Question:** Python pre-commit vs alternatives?
**Sources:** lefthook repo, prek repo, pre-commit docs
**Findings:** lefthook is Go binary (no runtime deps), growing adoption, fast
**Conclusion:** lefthook matches user preference for no Python deps

### Dead-End Paths
#### Rudra for CI
**Why explored:** Found 264 memory safety bugs across crates.io
**Why abandoned:** Research project, pinned to nightly-2021-10-21, not maintained for current Rust

#### SonarQube
**Why explored:** Popular enterprise SAST
**Why abandoned:** Overkill for single-maintainer OSS project, requires server infrastructure

### Open Concerns Raised
- None raised yet — design approved on first pass
