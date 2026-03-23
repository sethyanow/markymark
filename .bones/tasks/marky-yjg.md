---
id: marky-yjg
title: Run baseline security scan and configure cargo-deny
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-84l
---



## Goal
Run all 4 security tools locally to establish baseline findings. Configure deny.toml based on actual dependency licenses. Document findings for triage.

## Implementation

1. Install tools locally
   - cargo install cargo-audit
   - cargo install cargo-deny
   - Install semgrep (brew install semgrep)

2. Run cargo-audit
   - cargo audit
   - Document any known CVEs in dependencies
   - Note: tree-sitter and tokio are primary dep chains to watch

3. Run cargo-deny init and configure deny.toml
   - cargo deny init
   - Review actual licenses: cargo deny list
   - Configure license allowlist based on real deps (MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib)
   - Enable advisory checks
   - Enable source checks (crates.io only)
   - Run: cargo deny check

4. Run Semgrep with built-in Rust rules
   - semgrep --config=p/rust .
   - Document findings by severity
   - Note false positives for custom rule tuning

5. Run CodeQL locally (optional, mostly for CI)
   - If codeql CLI available: codeql database create --language=rust
   - Otherwise, document that CodeQL will be CI-only

6. Document baseline in beads notes
   - Total findings per tool
   - Critical/high findings needing immediate fix
   - False positives needing suppression
   - License exceptions needed for deny.toml

## Success Criteria
- [ ] cargo-audit runs clean or findings documented
- [ ] deny.toml exists with license allowlist matching actual deps
- [ ] cargo deny check passes (or findings documented)
- [ ] Semgrep baseline findings documented
- [ ] Baseline summary written to beads notes on this task
- [ ] All existing tests still pass

## Design

## Goal
Run all 4 security tools locally to establish baseline findings. Configure deny.toml based on actual dependency licenses. Document findings in beads notes on this task for triage.

## Effort Estimate
4-6 hours

## Implementation

1. Install tools locally
   - cargo install cargo-audit
   - cargo install cargo-deny
   - brew install semgrep
   - Verify each: cargo audit --version, cargo deny --version, semgrep --version

2. Run cargo-audit
   - cargo audit
   - Record output verbatim in beads notes
   - For each finding: note CVE ID, affected crate, severity, whether direct or transitive dep
   - If critical/high CVE in direct dep with no fix: flag for immediate attention

3. Configure deny.toml
   - cargo deny init (creates template deny.toml)
   - Run cargo deny list to see actual licenses in dep tree
   - Configure [licenses] section:
     - allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Unicode-DFS-2016", "Zlib", "0BSD", "BSL-1.0"]
     - confidence-threshold = 0.8
   - Configure [advisories] section:
     - db-path = "~/.cargo/advisory-db"
     - vulnerability = "warn" (advisory-only per epic requirement)
     - unmaintained = "warn"
   - Configure [sources] section:
     - unknown-registry = "deny"
     - unknown-git = "deny"
     - allow-registry = ["https://github.com/rust-lang/crates.io-index"]
   - Configure [bans] section:
     - multiple-versions = "warn" (start with warn, tighten later)
   - Run: cargo deny check
   - If license failures: review each, add to allow list only if genuinely acceptable (document rationale)
   - If advisory failures: document each, note if fix available

4. Run Semgrep baseline
   - semgrep --config=p/rust --json --output=semgrep-baseline.json .
   - semgrep --config=p/rust . (human-readable for review)
   - For each finding: note rule ID, severity, file:line, whether true positive or false positive
   - If >50 findings: focus on ERROR/WARNING severity only for baseline
   - Delete semgrep-baseline.json after review (don't commit)

5. CodeQL baseline (skip locally)
   - CodeQL requires GitHub infrastructure for Rust; skip local run
   - Document: CodeQL will be CI-only via github/codeql-action

6. Document baseline summary
   - Update this beads issue notes with structured summary:
     ```
     ## Baseline Scan Results (DATE)
     ### cargo-audit: X findings (Y critical, Z high)
     - CVE-XXXX: crate-name (severity) — [fix available|no fix]
     ### cargo-deny: X findings
     - License: N issues (list exceptions added)
     - Advisory: N issues
     - Sources: N issues
     ### Semgrep: X findings (Y error, Z warning)
     - rule-id: N occurrences — [true positive|false positive|needs review]
     ### Action Items
     - [Critical items needing immediate fix]
     - [False positives to suppress in custom rules]
     ```

## Success Criteria
- [ ] cargo audit runs and output recorded in beads notes (zero findings = clean, N findings = documented with CVE IDs)
- [ ] deny.toml exists at project root with all 4 sections configured ([licenses], [advisories], [sources], [bans])
- [ ] cargo deny check runs (warn-only mode) — any failures documented with rationale
- [ ] Semgrep baseline findings documented in beads notes with severity and true/false positive assessment
- [ ] Structured baseline summary written to beads notes on this issue (format above)
- [ ] All existing tests still pass: cargo test --workspace
- [ ] deny.toml validates: cargo deny check (exit code 0, or only warnings documented)

## Anti-Patterns (FORBIDDEN)
- ❌ NO adding licenses to deny.toml allow list without reviewing what crate uses them (blind allowlisting defeats the purpose)
- ❌ NO committing semgrep-baseline.json or other scan output files (ephemeral data, not source)
- ❌ NO suppressing findings without documenting why in beads notes (suppressed findings must have rationale)
- ❌ NO modifying existing source code during baseline scan (this task is observe-only; fixes are separate tasks)
- ❌ NO setting advisory/vulnerability checks to "deny" mode (epic requires advisory-only)

## Key Considerations (SRE Review)

**Edge Case: Critical CVE with no fix**
- If cargo-audit finds a critical CVE in a direct dependency (e.g., tree-sitter, tokio) with no patch:
  - Document the CVE, affected versions, and exposure surface
  - Check if the vulnerable code path is actually exercised by markymark
  - Create a separate beads issue for remediation (don't block this task)
  - DO NOT add to cargo-audit ignore list without documenting why

**Edge Case: Non-standard SPDX identifiers**
- Some crates use non-standard license identifiers or custom licenses
  - cargo deny list shows actual licenses — review each before adding to allow
  - unicode-ident uses "Unicode-3.0" (valid SPDX but uncommon)
  - ring uses ISC (valid but less common)
  - If a license is truly unrecognizable: document and flag for manual review

**Edge Case: Noisy Semgrep baseline**
- If Semgrep produces >100 findings, don't try to triage every one
  - Focus on ERROR severity first
  - Group by rule ID to identify systematic patterns vs one-offs
  - Note: test files will likely trigger many false positives — filter with --exclude='**/tests/**' for initial assessment, then review unfiltered

**Edge Case: Tool installation failures**
- If brew not available: install semgrep via pip (pip install semgrep) or skip with documentation
- If cargo install fails: check rust toolchain version, try --locked flag
- Document any installation issues for CI workflow setup (different environment)

**Reference: Similar deny.toml configs**
- Tokio project deny.toml: standard Rust ecosystem patterns
- Use cargo deny list --layout=crate to understand which crates bring which licenses
