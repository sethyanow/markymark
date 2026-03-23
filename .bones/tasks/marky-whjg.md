---
id: marky-whjg
title: Consolidate TempWorkspace test helper into shared module
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

TempWorkspace is duplicated in 4 test files: diagnostics_tests.rs, runtime_engine_tests.rs, search_symbols_tests.rs, runtime_tools.rs. Extract to a shared test-utils module. Source: CodeRabbit nitpick on PR #38.
