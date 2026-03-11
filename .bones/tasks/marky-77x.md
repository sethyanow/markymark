---
id: marky-77x
title: 'Task 2: Implement true wiki_links selective merge in incremental path'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-z7e]
parent: marky-77i
---




## Design

## Goal
Replace current incremental wiki-link scaffolding (which still rebuilds via `DocumentIndex::from_ast`) with true selective wiki-link updates: purge intersecting ranges, re-extract affected links, validate neighbors, and merge with unchanged entries while preserving full rebuild for headings/TOC/outline.

## Context
- Completed marky-z7e established: pending edit tracking, incremental entrypoint wiring, and regression tests.
- Current `build_markdown_index_incremental()` computes overlap but still calls full `DocumentIndex::from_ast(ast)`.
- Epic requires real selective extractor updates for 10x target.

## Implementation
1. Add index-layer helper(s) to support wiki-link selective merge using edit ranges + old entries.
2. Ensure unchanged wiki-links can be reused logically (content/range parity) while changed neighborhood re-extracts from new AST.
3. Keep headings/TOC/outline full rebuild path unchanged.
4. Integrate helper into LSP incremental orchestration.
5. Add/expand tests for overlap math, partial overlaps, multi-line edits, and neighbor-window behavior.
6. Add benchmark test/measurement for wiki-link-heavy markdown edit case and compare with full rebuild baseline.

## SRE Refinement (Edge/Failure Cases)
- Overlapping edits in same region should not duplicate links in merged output.
- Boundary overlap: edit touching start/end of link range must force re-extract.
- Multi-line edits with same-line wiki-links before/after changed line should not be falsely purged.
- Full-line delete with adjacent links should preserve ordering after merge.
- Empty pending edits should skip selective processing with zero behavior change.
- Large documents with sparse links should avoid O(n*m) hotspots.
- Fallback path: on merge inconsistency, safe full rebuild must preserve correctness.

## Success Criteria
- [ ] Incremental wiki-link path no longer always delegates to full `DocumentIndex::from_ast`.
- [ ] Purge + re-extract + neighbor-validate + merge implemented and covered by tests.
- [ ] Existing + new wiki-link incremental tests pass.
- [ ] Benchmark evidence shows measurable improvement vs full rebuild for wiki-link-focused edit scenario.
- [ ] fmt/clippy/tests all green.

## Anti-Patterns
- NO AST tree-diff layer.
- NO incremental headings/TOC/outline logic.
- NO correctness trade-offs for speed.
