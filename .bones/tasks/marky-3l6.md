---
id: marky-3l6
title: 'Triage remaining 10 code review comments on PR #21'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

## Context

PR #21 (feature/mark-next) has 10 additional code review comments on parser/LSP files that need triage and resolution. These were not security findings but may contain important implementation feedback.

## Files with Comments

1. **markymark-parser/src/ast.rs** - 4 comments
2. **markymark-lsp/src/state.rs** - 2 comments
3. **markymark-lsp/tests/state_tests.rs** - 1 comment
4. **markymark-parser/benches/incremental.rs** - 1 comment
5. **markymark-parser/src/lib.rs** - 1 comment
6. **markymark-parser/tests/tree_sitter_integration.rs** - 1 comment

## Action Items

- [x] Parse comment bodies from pr21-comments.json for these files
- [x] Categorize by type (suggestion, bug, question, etc.)
- [x] Determine which require code changes vs responses
- [ ] Address or respond to each comment
- [ ] Update PR #21 with responses/fixes

## Source Data

All comments available in:
- pr21-comments.json (raw API data)
- Can filter with: jq '.[] | select(.path != "markymark-index/src/document.rs" and .type == "review")' pr21-comments.json

## Related

- Parent triage: See pr21-triage-detailed.md
- Security audit: marky-ktt (unsafe transmute audit)
- PR: https://github.com/sethyanow/markymark/pull/21
