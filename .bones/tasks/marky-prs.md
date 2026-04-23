---
id: marky-prs
title: Fix parse_block_tree_only normalization leak (utf8_text panic in is_logseq_heading)
status: closed
type: bug
priority: 1
parent: marky-p88
---






## Context

Closed in the 2026-04-20 session. Retained here so the work can be reviewed alongside the rest of the epic.

## Root cause (confirmed)

`markymark_parser::Parser::parse_block_tree_only(source)` at `markymark-parser/src/lib.rs:45` appends `\n` to the source when it lacks a trailing newline (required for tree-sitter-md's block grammar to parse cleanly), parses against the normalized buffer, and returns ONLY the Tree.

The sole caller, `markymark_index::document::from_engine::extract_content_blocks_inner` at `markymark-index/src/document/from_engine.rs:40`, retained its original un-normalized source and passed it to both `collect_blocks(root, source, ...)` and thereafter to `is_logseq_heading(node, source)`.

`is_logseq_heading` at line 112-116 called `node.utf8_text(source.as_bytes())`. Tree-sitter's implementation at `binding_rust/lib.rs:2010` does `str::from_utf8(&source[self.start_byte()..self.end_byte()])` with no bounds check. When a list_item node's end_byte was exactly `source.len() + 1` (the appended normalization byte), the slice panicked: `range end index N+1 out of range for slice of length N`.

The panic was caught by `std::panic::catch_unwind` in `extract_content_blocks`, which returned `Vec::new()`. User-visible effect: every content block in the affected file was silently dropped from the index.

## Evidence

- Direct reproducer: a file containing only `"- # Heading"` (11 bytes, no trailing newline) panics with `range end index 12 out of range for slice of length 11`.
- Full stack trace captured in session: frames 18→44 → tree-sitter::Node::utf8_text → is_logseq_heading → collect_blocks (recursive) → extract_content_blocks_inner → catch_unwind → extract_content_blocks → from_engine_result_with_source → index_from_engine_result → build_markdown_index_via_engine → index_root_into_realm → RuntimeEngine::from_workspace_roots.
- Torture harness (`/tmp/mm_torture.py`, 72 cases) identified four distinct triggering inputs: `- # Heading`, `- a\n  - b\n    - c\n      - d`, and the `.markdown` variants — all lacking trailing newlines.

## Fix applied

Extracted a single public helper `markymark_parser::normalize_block_source(&str) -> Cow<str>` in `markymark-parser/src/lib.rs`. Four previously-duplicated copies of the normalization logic collapsed into one call site each:

- `parse_block_tree_only` (markymark-parser/src/lib.rs:45)
- `parse_tree_only` (markymark-parser/src/lib.rs)
- `parse_with_old_tree` (markymark-parser/src/lib.rs:93)
- `DocumentIndex::from_engine_result_with_source` (markymark-index/src/document/from_engine.rs) — new caller, normalizes at entry so `DocumentOwner.source_text`, the tree walked by `collect_blocks`, and `utf8_text` slices all agree on a single byte sequence.

## Regression tests

New Bazel target `//markymark-index:parse_robustness_test` wired via `markymark-index/BUILD.bazel`. Test file: `markymark-index/tests/parse_robustness.rs` — 8 tests covering:

- Logseq-style list-heading without trailing newline
- Paragraph + Logseq-heading without trailing newline (preceding-block drop)
- Four-level nested list without trailing newline
- Trailing paragraph with no final newline (`block_text()` empty bug)
- Single-byte list marker
- CRLF line endings with trailing list item, no final EOL
- UTF-8 BOM + list item, no trailing newline
- 4-space indented list item, no trailing newline

All 8 pass in both cargo and Bazel. Torture harness passes 72/72 with 0 panics.

## Verification

- `bazel test //...` — 8 targets, all green
- `bazel-bin/markymark-cli/markymark --mcp <repro-dir>` — exits 0, empty stderr, no panic text
- `/tmp/mm_torture.py` — 72/72 clean

## Files changed

- `markymark-parser/src/lib.rs` — new `normalize_block_source` helper; removed 3 duplicate normalization blocks.
- `markymark-index/src/document/from_engine.rs` — normalize at `from_engine_result_with_source` entry; updated `catch_unwind` comment to note defence-in-depth positioning.
- `markymark-index/BUILD.bazel` — added `parse_robustness_test` target; note that other integration tests still unwired (tracked separately as marky-p88.2).
- `markymark-index/tests/parse_robustness.rs` — new regression test file.

## Status

Ready to close. This task exists primarily for review traceability — fix was applied and verified in-session. Remaining latent issues tracked under sibling tasks in epic marky-p88.

## Success Criteria

- [x] Repro identified and documented
- [x] Root cause traced to specific file:line
- [x] Fix applied with shared helper (eliminates 4-way duplication)
- [x] 8 regression tests (all pass)
- [x] 72-case torture harness (0 panics)
- [x] Full Bazel test suite green
- [x] Live MCP smoke test clean

## Log

- [2026-04-20T18:13:37Z] [Seth] Session 2026-04-20: /debugging-with-tools + /test-driven-development. Root cause confirmed via LSP-assisted trace + 72-case torture harness. Fix: extract normalize_block_source helper in markymark-parser; collapse 4 duplicate normalization sites to 1 helper + 4 callers; normalize at from_engine_result_with_source entry so source_text / tree-walk / utf8_text all agree. 8 regression tests (markymark-index/tests/parse_robustness.rs, wired as //markymark-index:parse_robustness_test). bazel test //... green. Torture 72/72 clean. See skeleton for full detail.
