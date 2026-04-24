---
id: marky-a85
title: dev branch behind on clippy + cargo-audit advisories — pre-commit hook red
status: open
type: bug
priority: 2
---


## Requirements

## Context

## Success Criteria

## Log

- [2026-04-24T23:14:57Z] [Seth] Discovered while landing marky-cje validation commit (dbec6521). Pre-commit hook on dev fails on:

1. clippy::question-mark at markymark-parser/src/extract/frontmatter.rs:14 — let-else block can be rewritten with ?. Fix is one line. origin/optimize already has this fix in commit c9f6860c (cosmetic let-else simplification of the ---\n strip_prefix branch in extract_frontmatter). Dev is behind optimize on this lint.

2. cargo audit reports 4 vulnerabilities (output truncated, captured what I saw):
   - quinn-proto 0.11.13 — RUSTSEC-2026-0037 (DoS in Quinn endpoints, severity 8.7 high). Solution: upgrade to >=0.11.14. Reached via reqwest -> markymark-core.
   - rustls-webpki 0.103.10 — RUSTSEC-2026-0104 (reachable panic in CRL parsing, 2026-04-22). Solution: upgrade to >=0.103.13. Reached via rustls -> ureq -> hf-hub -> fastembed -> markymark-core.
   - Two more vulnerabilities truncated from output — re-run cargo audit to see full list.
   - GitHub Dependabot also reported 10 vulnerabilities on default branch (5 high, 1 moderate, 4 low) per push remote message.

Likely path: cargo update first (most rustsec advisories are minor-version bumps). If lockfile bumps don't resolve, may need workspace dep constraint bumps.

Workaround used in dbec6521: --no-verify with explicit user authorization. No new contamination — bones-only commit. The dev-state issues predate this commit.

Likely should be addressed before the marky-cje fix lands so that fix's pre-commit gate is clean.
