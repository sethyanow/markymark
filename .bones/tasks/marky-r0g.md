---
id: marky-r0g
title: 'fix(tests): replace std::env::temp_dir() with tempfile::tempdir() crate'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

GH Advanced Security (Semgrep rust.lang.security.temp-dir) flagged temp_dir() usage in markymark-mcp/src/engine/tests.rs (lines 6, 205, 315) and markymark-mcp/src/pattern/tests.rs (line 8). PID-based paths are TOCTOU racy. tempfile crate is already added as dev-dependency in PR #36. Replace manual create_dir_all+remove_dir_all pattern with tempfile::tempdir(); update call sites to use .path().
