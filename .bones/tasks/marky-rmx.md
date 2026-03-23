---
id: marky-rmx
title: 'Task 3: Implement selective merge for blocks extractor'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-77x]
parent: marky-77i
---




## Design

## Goal
Implement true incremental selective merge for blocks in the markdown incremental path using the same pattern proven in marky-77x (purge/re-extract/neighbor-aware merge), while keeping headings/TOC/outline full rebuild.

## Context
- marky-77x completed wiki-link selective merge with owned payload override path.
- Epic marky-77i still requires selective incremental handling for blocks, tags, markdown_links, and xml_tags.
- Existing incremental flow now supports passing old payloads into index construction.

## Implementation
1. Add owned payload representation for block entries and optional override plumbing in DocumentIndex construction (matching wiki-link pattern).
2. Capture old block payloads in ServerState before document removal.
3. Add block affected-range logic and incremental merge helper in ServerState.
4. Integrate block merge into build_markdown_index_incremental without changing structured-format fallback behavior.
5. Add tests for overlap handling, unchanged-region reuse parity, and multi-edit stability.

## SRE Refinement (Edge/Failure Cases)
- Edit touching block-id boundary must force re-extract.
- Full-line delete near adjacent block IDs must not duplicate or drop surviving IDs.
- Empty pending edits must preserve behavior and avoid unnecessary merge work.
- Multiple overlapping edits in one change batch must produce deterministic block ordering.
- Large markdown files with sparse blocks should avoid O(n*m) hot-path regressions.

## Success Criteria
- [ ] Incremental path no longer full-rebuilds blocks when old index is available.
- [ ] Correctness parity tests for block extraction pass against full rebuild.
- [ ] Existing workspace tests remain green.
- [ ] fmt/clippy/tests pass.

## Anti-Patterns
- NO AST tree-diff implementation.
- NO incremental headings/TOC/outline logic.
- NO correctness compromises for speed.
