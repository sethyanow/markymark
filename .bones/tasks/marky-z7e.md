---
id: marky-z7e
title: 'Task 1: Implement incremental wiki_links as proof-of-concept'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-77i]
---



## Design

## Goal

Implement incremental indexing infrastructure with wiki_links as proof-of-concept extractor. This task establishes the pattern: ServerState tracks InputEdit ranges, build_markdown_index_incremental() conditionally updates wiki_links using range-based purge + neighbor validation, correctness test proves incremental matches full rebuild.

## Effort Estimate

**10-14 hours** (borderline large, monitor during execution)
- Infrastructure (pending_edits): 2-3 hours
- update_wiki_links implementation: 4-5 hours
- TDD tests (3 tests): 2-3 hours
- Debug, tune, validate: 2-3 hours

## Implementation

### 1. Study existing code

**Current indexing flow:**
- markymark-lsp/src/state.rs:208-228 - `build_markdown_index()` calls Parser::parse() then DocumentIndex::from_ast()
- markymark-lsp/src/state.rs:145-170 - `apply_document_changes()` computes InputEdit for tree-sitter, doesn't save them
- markymark-index/src/document.rs:202-337 - `DocumentIndex::from_ast()` always rebuilds all 9 indexes

**Similar patterns in codebase:**
- markymark-lsp/src/state.rs:160 - md_tree.edit(&input_edit) shows how to iterate InputEdit
- markymark-parser/src/lib.rs:73-89 - parse_with_old_tree() shows reuse pattern for incremental

### 2. Write tests first (TDD)

**Test 1: incremental_wiki_links_matches_full_rebuild**
- Setup: Parse doc with 5 wiki links
- Edit: Change one link target `[[Page]]` → `[[Other]]`
- Assert: incremental update produces identical wiki_links to full rebuild
- **What bug it catches:** Incremental produces different result than full rebuild (correctness violation)

**Test 2: wiki_links_unchanged_sections_reused (STRENGTHENED)**
- Setup: Parse doc with 3 wiki links at known positions: (10,20), (500,510), (1000,1010)
- Edit: Change bytes 400-600 (middle section only)
- Assert:
  - First link (10,20) has identical target AND position in incremental result (content reuse)
  - Third link (1000,1010) has identical target AND position in incremental result (content reuse)
  - Second link (500,510) is re-extracted (in changed range)
- **What bug it catches:** Unchanged sections are incorrectly re-extracted (performance bug, wasted work)
- **Note:** Do NOT check memory addresses (unreliable due to cloning). Check content + position equality.

**Test 3: wiki_links_neighbor_validation**
- Setup: Parse doc with adjacent wiki links `[[A]][[B]]` at bytes 100-110
- Edit: Insert text between them at byte 105: `[[A]] text [[B]]`
- Assert: Both links re-extracted with correct new positions
- **What bug it catches:** Neighbor validation skipped, positions become stale

**Test 4: edge_case_empty_pending_edits (NEW)**
- Setup: Parse doc with 3 wiki links
- Edit: Call build_markdown_index_incremental with empty pending_edits
- Assert: All 3 wiki links reused (short-circuit optimization works)
- **What bug it catches:** Unnecessary work when no edits occurred

**Test 5: edge_case_all_links_in_changed_range (NEW)**
- Setup: Parse doc with 2 wiki links both in bytes 0-100
- Edit: Replace entire range 0-100
- Assert: Both links re-extracted, result matches full rebuild
- **What bug it catches:** Full rebuild case within incremental path

### 3. Implementation checklist

**markymark-lsp/src/state.rs:**
- [ ] Line ~80 - Add `pending_edits: Vec<InputEdit>` field to ServerState
- [ ] Line ~90 - Initialize `pending_edits: Vec::new()` in ServerState::new()
- [ ] Line ~145-170 - Modify `apply_document_changes()`:
  - After computing InputEdit, push to self.pending_edits
  - Do NOT clear pending_edits here (cleared after reindex)
- [ ] Line ~210-240 - Add `build_markdown_index_incremental()` method:
  - Takes old_index: &DocumentIndex, new_ast: Ast, pending_edits: &[InputEdit]
  - **Short-circuit:** If pending_edits.is_empty(), clone old wiki_links and return early
  - Always rebuild headings/TOC/outline (call existing from_ast helpers)
  - Check if wiki_links needs update: `ranges_intersect(&old_index.wiki_links, pending_edits)`
  - If yes: call `update_wiki_links(old_index, &new_ast, pending_edits)`
  - If no: clone old_index.wiki_links
  - Return new DocumentIndex
- [ ] Line ~210 - Modify `build_markdown_index()`:
  - Check if old index exists AND pending_edits not empty
  - If yes: call build_markdown_index_incremental, then CLEAR pending_edits
  - If no: full rebuild via from_ast()

**markymark-index/src/document.rs:**
- [ ] Line ~400 (new) - Add `ranges_intersect()` helper:
  - Signature: `fn ranges_intersect(entries: &[WikiLinkEntry], edits: &[InputEdit]) -> bool`
  - Algorithm: For each entry, check if entry.range.start < edit.new_end_byte AND entry.range.end > edit.start_byte for any edit
  - Return true on first intersection (early exit)
- [ ] Line ~420 (new) - Add `update_wiki_links()` helper:
  - Signature: `fn update_wiki_links<'arena>(old: &[WikiLinkEntry<'arena>], ast: &Ast<'arena>, edits: &[InputEdit]) -> Vec<WikiLinkEntry<'arena>>`
  - **Step 1 - Purge intersecting:** Filter old entries, keep only those where `!ranges_intersect(&[entry], edits)`
  - **Step 2 - Extract from changed:** Call `ast.extract_wiki_links()` to get all links, filter to only those in changed ranges
  - **Step 3 - Validate neighbors:** For surviving old entries within NEIGHBOR_DISTANCE (100 bytes) of any edit, re-extract from ast by position and verify content matches. If mismatch, discard old and use new.
  - **Step 4 - Merge:** Combine surviving old + new extracted, sort by position, return
  - **Arena note:** New entries from ast.extract_wiki_links() share same 'arena lifetime as old entries

**markymark-lsp/tests/state_tests.rs:**
- [ ] Line ~500 (new) - Test `incremental_wiki_links_matches_full_rebuild()` (Test 1 spec above)
- [ ] Line ~550 (new) - Test `wiki_links_unchanged_sections_reused()` (Test 2 spec above - STRENGTHENED)
- [ ] Line ~600 (new) - Test `wiki_links_neighbor_validation()` (Test 3 spec above)
- [ ] Line ~650 (new) - Test `edge_case_empty_pending_edits()` (Test 4 spec above)
- [ ] Line ~700 (new) - Test `edge_case_all_links_in_changed_range()` (Test 5 spec above)

## Success Criteria

- [ ] ServerState.pending_edits field added, initialized, populated in apply_document_changes(), cleared after reindex
- [ ] build_markdown_index_incremental() implemented with short-circuit for empty edits
- [ ] ranges_intersect() helper implemented with early-exit optimization
- [ ] update_wiki_links() helper implemented with 4-step merge (purge, extract, validate, merge)
- [ ] 5 TDD tests pass (3 original + 2 new edge cases)
- [ ] All existing workspace tests still pass (468 tests across 54 suites): `cargo test --workspace`
- [ ] Zero clippy warnings: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Fmt clean: `cargo fmt --all --check`
- [ ] Benchmark: single wiki link change in 10KB doc ≥2x faster than full rebuild (proof of incremental benefit)

## Anti-Patterns (FORBIDDEN)

- ❌ NO unwrap/expect in incremental update path (reason: invalid ranges could panic - use ? or explicit error handling with Result)
- ❌ NO panicking on out-of-bounds ranges (reason: defensive programming - clamp or validate ranges before use)
- ❌ NO skipping correctness tests (reason: incremental must match full rebuild - optimization without correctness is a bug)
- ❌ NO arena lifetime violations (reason: new wiki_links must use same 'arena as old entries - extract from ast which owns the arena)
- ❌ NO leaving pending_edits accumulated after reindex (reason: causes incorrect incremental updates on next change - must clear after use)
- ❌ NO Test 2 checking memory addresses (reason: cloning changes addresses even when content reused - check content equality instead)

## Key Considerations (ADDED BY SRE REVIEW)

### Edge Case: Empty pending_edits
**Problem:** What if no edits accumulated between reindex calls?
**Solution:** Short-circuit in build_markdown_index_incremental() - if pending_edits.is_empty(), clone old wiki_links and return immediately. Prevents unnecessary intersection checks.
**Test:** Test 4 validates this optimization.

### Edge Case: Overlapping InputEdit ranges
**Problem:** User types fast, multiple edits in same region (e.g., "ab" → "abc" → "abcd")
**Solution:** Merge algorithm handles overlapping ranges via union - if any edit intersects an entry, it's purged. No special handling needed.
**Test:** Implicit in Test 1 (edit changes one link, others unaffected).

### Edge Case: Wiki link spans edit boundary
**Problem:** Link at bytes 90-110, edit at bytes 95-105 (link partially in changed range)
**Solution:** ranges_intersect() uses overlap test (start < edit.end AND end > edit.start), so partial overlap → purge → re-extract entire link.
**Test:** Test 3 validates neighbor detection.

### Edge Case: Edit at document start/end
**Problem:** Neighbor validation at boundaries (bytes 0-100 or end-100 to end)
**Solution:** Neighbor validation checks "within ±100 bytes" - at boundaries, one side will have no neighbors (correct behavior).
**Test:** Add explicit boundary tests in Test 5.

### Edge Case: No wiki links
**Problem:** Empty wiki_links vector, any operation?
**Solution:** ranges_intersect() returns false for empty slice, update_wiki_links() returns empty Vec. Works correctly.
**Test:** Implicit (empty case is trivial).

### Edge Case: All wiki links in changed range
**Problem:** No reuse benefit (e.g., entire doc replaced)
**Solution:** Purge step removes all old entries, extract step gets all new links. Equivalent to full rebuild (correct).
**Test:** Test 5 validates this case.

### Arena Lifetime Safety
**Problem:** wiki_links are &'arena refs. New entries from re-extraction must share same arena.
**Solution:** ast.extract_wiki_links() returns entries borrowing from ast's arena. Since DocumentIndex takes ownership of this arena (via from_ast pattern), all entries share same lifetime. No separate allocation needed.
**Reference:** markymark-index/src/document.rs:202-337 - from_ast() ownership transfer pattern.

### Concurrent LSP Requests
**Problem:** What if another did_change arrives during reindex?
**Solution:** tower-lsp serializes requests per document URI. Pending_edits is not shared across URIs. No concurrency issue within same document.
**Reference:** tower-lsp async traits enforce sequential processing per resource.

### Performance: Large pending_edits
**Problem:** User types 1000 chars without triggering reindex - large pending_edits vector
**Solution:** Acceptable - InputEdit is small (48 bytes), 1000 edits = 48KB. Merge algorithm is O(n+m) where n=old entries, m=edits. For typical markdown (50-100 links), this is fast.
**Future:** Could batch edits if needed, but not in this task.

## Notes

- Other 4 extractors (blocks, tags, markdown_links, xml_tags) stay full rebuild for now - Task 2+ will replicate this pattern
- Headings/TOC/outline always rebuild (accept O(headings) cost per epic design)
- Neighbor validation distance: ±100 bytes (tunable constant)
- If correctness test fails, incremental logic has a bug - fix before moving to other extractors
- Clear pending_edits AFTER successful reindex, not before (allows retry on failure)
