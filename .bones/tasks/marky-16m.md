---
id: marky-16m
title: 'GHAS: temp-dir security + boundary-unwrap findings in runtime_engine.rs tests'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

GitHub Advanced Security / Semgrep findings from PR #28.

1. rust.lang.security.temp-dir.temp-dir (line 709): make_temp_realm_dir() uses std::env::temp_dir() which is shared/predictable. Tests should use the 'tempfile' crate (TempDir) for secure, auto-cleaned temp dirs.

2. semgrep.markymark.rust.boundary-unwrap (lines 736, 767, 827, 871): .unwrap() called at protocol boundaries in tests. Test code should use expect() with descriptive messages at minimum; production protocol boundary code must return structured errors.

Refs: GHAS code-scanning alerts #123-127 on markymark repo.
