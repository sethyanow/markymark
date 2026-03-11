---
id: marky-ulw
title: Move incremental tests from state.rs to incremental.rs to get state.rs under 1000 lines
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-gb1]
---


state.rs is 1359 lines (1025 production + 334 tests). The compile fixes in marky-gb1 resolved all errors but didn't reduce the line count meaningfully (-31 net). The ~13 incremental tests (range_is_after_edit_start, range_within_neighbor_window, wiki_links_need_update, blocks_need_update, merge_incremental_blocks, build_markdown_index_incremental_blocks_parity, plus helpers like make_block_owned) currently live in state::tests but test functions that now live in incremental.rs. Move them to incremental::tests. This should drop state.rs to ~1025 lines (production only) and keep incremental.rs well under its 500-line budget (~747 + 334 = ~1081 total, but tests don't count toward the limit). No logic changes needed — just cut/paste tests and update any imports.
