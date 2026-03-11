---
id: marky-pvr
title: 'Optimize: deduplicate file reads in SemanticSearch preview extraction'
status: open
type: feature
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3.12, marky-36a]
---



## Design

## Goal
Replace O(top_k) file reads in preview extraction with O(unique_docs) reads by grouping results by doc_uri before calling preview_for_range.

## Context
Profiling (marky-8s3.12) showed:
- BufReader streaming is 40-75% SLOWER than fs::read_to_string (warm cache) — do NOT use streaming
- Top_k=10 across 10 unique 500KB files costs ~3.8ms total in preview I/O
- When results cluster in few files (common for large structured docs), current code reads same file N times

## Implementation
1. Refactor preview_for_range to accept source: &str instead of re-reading from URI
   - New private fn: fn extract_preview(source: &str, range: Range, fallback: &str) -> String
   - Keep preview_for_range as a thin wrapper: read file, call extract_preview
2. In SemanticSearch arm of execute(), deduplicate file reads:
   - Collect results into Vec first (already done)
   - Build HashMap<DocumentUri, String> reading each unique file once
   - Map results using the cached sources
3. Update tests to cover deduplication behavior

## Success Criteria
- [ ] extract_preview(source, range, fallback) extracts correct preview from pre-loaded source
- [ ] SemanticSearch reads each unique doc_uri exactly once regardless of top_k
- [ ] All existing semantic-search tests pass (requires marky-36a fixed first)
- [ ] Test: 10 results from same 1MB file completes faster than 10 results from 10 different files
- [ ] No regression in single-result case (one file, one read)

## Implementation Notes
- Do NOT switch to BufReader (profiling shows 40-75% slower for warm cache)
- The deduplication is O(top_k) extra HashMap ops — negligible vs file I/O
- This is blocked on marky-36a (EmbeddingIndex !Send) to run with semantic-search feature
