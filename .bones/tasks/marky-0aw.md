---
id: marky-0aw
title: 'Extraction parity tests: Zig SIMD vs tree-sitter'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0oz]
---


Write comprehensive parity tests comparing Tier 1 (Zig scan) vs Tier 2 (tree-sitter) extraction on: (1) individual test documents with known structure, (2) the project's own docs/ directory (~50 files), (3) if available, a larger corpus. Measure: heading count match rate, link count match rate, false positive rate from code blocks. Track false positive rate — if < 5%, complement strategy is validated.

## Design

## Goal
Write comprehensive parity tests comparing Tier 1 (Zig SIMD scan via ScanBackend) vs Tier 2 (tree-sitter extraction) on individual test documents, the project's own docs/ directory (~50 files), and optionally a larger corpus. Measure match rates and false positive rates. If false positive rate < 5%, the complement strategy is validated.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] Parity test suite with 10+ hand-crafted test documents covering all element types
- [ ] Automated parity test over docs/ directory (~50 files) comparing heading/link/tag/block counts
- [ ] False positive rate measured and reported for each element type
- [ ] Overall false positive rate < 5% validates complement strategy
- [ ] Code block false positives specifically tracked (primary source of FPs)
- [ ] Test results written to docs/benchmarks/extraction-parity.md
- [ ] cargo test --features zig-kernels -- --test-threads=1 parity passes all tests
- [ ] Known differences documented (setext headings, frontmatter) with rationale

## Implementation Checklist
- [ ] Create tests/extraction_parity.rs integration test file
- [ ] Write 10+ test documents as string constants: simple, complex, code-heavy, wiki-link-heavy, tag-heavy
- [ ] For each test doc: run from_ast() and from_scan(), compare heading counts
- [ ] For each test doc: compare link counts (markdown + wiki separately)
- [ ] For each test doc: compare tag counts
- [ ] For each test doc: compare block ID counts
- [ ] Automated docs/ scan: iterate docs/**/*.md, compare both extraction paths
- [ ] Track per-element false positive rate: (zig_count - tree_sitter_count) / tree_sitter_count
- [ ] Track per-element false negative rate: elements tree-sitter finds that Zig misses
- [ ] Generate summary markdown report
- [ ] Document known gaps: setext headings (Zig misses), frontmatter (Zig misses)

## Edge Cases
- Documents with only code blocks: high expected false positive rate from Zig
- Documents with no structural elements: both should return 0 for everything
- Documents with frontmatter: Zig may scan into YAML, tree-sitter skips it
- Documents with HTML blocks: neither Zig nor tree-sitter may agree
- Documents with nested lists containing links: tree-sitter contextual, Zig positional
- Setext-style headings: Zig misses them (ATX only), document as known gap
- Very short documents (< 100 bytes): may have different overhead characteristics

## Anti-patterns
- NO ignoring false positives from code blocks (they must be measured and reported)
- NO only testing perfect-match documents (must test real-world messy markdown)
- NO hardcoding expected counts (compare the two backends dynamically)
- NO failing the test suite on known gaps (document them, don't assert equality where it's impossible)
- NO only measuring counts (verify offsets match too, at least spot-check)

## Error Handling
- File read failure in docs/ scan: skip file with warning, don't fail suite
- Tree-sitter parse failure: log and skip file
- ScanBackend failure: log and skip file
- Both return empty: valid result, not an error

## Test Specifications (what bug does each test catch?)
- test_parity_simple_headings: catches basic heading count divergence
- test_parity_simple_links: catches basic link count divergence
- test_parity_code_block_fps: catches and measures false positives from code blocks
- test_parity_wiki_links: catches wiki-link detection differences
- test_parity_tags: catches tag boundary detection differences
- test_parity_block_ids: catches block ID detection differences
- test_parity_setext_headings: documents known gap (Zig misses setext)
- test_parity_frontmatter: documents known gap (Zig may scan YAML)
- test_parity_docs_directory: catches regression across real project docs
- test_false_positive_rate_report: catches FP rate exceeding 5% threshold
- test_false_negative_rate_report: catches Zig missing elements that tree-sitter finds
- test_offset_spot_check: catches offset calculation differences between backends
