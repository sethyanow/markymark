---
id: marky-5qa
title: 'Validate/fix incremental indexing performance: benchmark shows <2x vs 10x target'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-77i]
---


## Problem

Epic marky-77i success criterion: 'single-char edit in 50KB markdown reindexes ≥10x faster than baseline'.

Final validation shows the existing benchmark (state_tests::benchmark_incremental_wiki_link_edit_faster_than_full_rebuild) gives ~1.05x speedup in release mode.

## Root Cause Analysis

range_is_after_edit_start() marks every wiki link AT OR AFTER the edit position as 'affected'. This means:
- For any edit that is NOT after the last wiki link in the document → wiki_links_need_update = true
- When needs_update = true: full re-extraction via extract_wiki_links_owned() + merge
- Net savings: zero for wiki_links extraction (still traverses AST)

The 10x speedup only materializes when edit is positioned AFTER ALL wiki links AND blocks AND markdown links AND xml tags in the document — an edge case in practice.

## What Does Pass

- All correctness tests (incremental_matches_full_rebuild, parity tests) ✅
- Stress test (100 sequential single-char edits) ✅
- Integration tests (hover, goto_definition, document_symbol) ✅
- Zero clippy warnings, fmt clean ✅

## Options

A) Revise the update decision: instead of 'any link at or after edit = affected', use byte-range intersection only (not position-after). Links BEFORE the edit that are not in the neighbor window are definitely correct and can be reused. Links AFTER the edit have correct relative positions (tree-sitter adjusts via InputEdit). Only links that INTERSECT the edit or are in the neighbor window need re-extraction.

B) Add a realistic 50KB benchmark and measure actual speedup. If typical editing (prose paragraph between wiki links) shows, say, 5x, document that actual number vs the 10x claim.

C) Accept current behavior (correctness is solid) and revise the epic's performance claim to reflect actual measured speedup.

## Next Steps

1. Construct a realistic 50KB benchmark (prose-heavy document with ~50 wiki links scattered)
2. Measure full vs incremental for: edit in prose, edit in wiki link, edit after all links
3. Based on results: either fix the update logic or revise the epic claim
4. Run with release profile (debug mode gives noisy results)

## Key File

markymark-lsp/src/incremental/mod.rs: wiki_links_need_update, wiki_link_affected_by_edits, range_is_after_edit_start
