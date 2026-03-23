---
id: marky-v8g
title: Implement TreeSitterScanBackend wrapping current extraction logic
status: open
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8ri]
parent: marky-qv6
---



## Design

## Goal
Implement TreeSitterScanBackend that wraps current markymark-parser extraction functions into the ScanBackend trait. This provides the baseline for parity testing against ZigScanBackend.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] TreeSitterScanBackend implements ScanBackend using markymark-parser functions
- [ ] Maps extract_headings -> scan_headings returning HeadingResult
- [ ] Maps extract_links -> scan_links returning LinkResult
- [ ] Maps extract_tags -> scan_tags returning TagResult
- [ ] Maps extract_block_ids -> scan_block_ids returning BlockIdResult
- [ ] estimate_tokens: simple whitespace-splitting approximation
- [ ] Compiles without zig-kernels feature (always available)
- [ ] cargo test -p markymark-core passes
- [ ] Parity test setup: same input, compare TreeSitter vs Zig results

## Edge Cases
- Parser extraction uses arena-allocated types, ScanBackend returns owned types — must convert
- Tree-sitter parse failure: return ScanError::InternalError
- Empty text: parser returns empty AST, scan returns empty results

## Anti-patterns
- NO creating a new Parser per scan call (cache or accept as param)
- NO exposing parser internals through ScanBackend trait

## Test Specifications
- test_tree_sitter_headings: catches incorrect AST-to-HeadingResult mapping
- test_tree_sitter_links: catches link type mapping errors
- test_tree_sitter_empty: catches panic on empty text
- test_parity_setup: catches result format differences between backends
