---
id: marky-0oz
title: Wire ScanBackend into DocumentIndex extraction path
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-qv6]
---





Modify markymark-index/src/document.rs to accept a ScanBackend for extraction when zig-kernels feature is enabled. DocumentIndex::from_ast uses tree-sitter (current). New DocumentIndex::from_scan(text, backend) uses ScanBackend. Both produce equivalent DocumentIndex. Add zig-kernels feature flag to markymark-index/Cargo.toml. Write parity tests: from_ast vs from_scan on 10+ test documents, compare heading/link/tag/block counts.

## Design

## Goal
Modify markymark-index/src/document.rs to accept a ScanBackend for extraction when zig-kernels feature is enabled. Current DocumentIndex::from_ast uses tree-sitter. New DocumentIndex::from_scan(text, backend) uses ScanBackend. Both produce equivalent DocumentIndex. Add zig-kernels feature flag to markymark-index/Cargo.toml. Write parity tests comparing from_ast vs from_scan.

## Effort Estimate
8-10 hours

## Success Criteria
- [ ] DocumentIndex::from_scan(text: &str, backend: &dyn ScanBackend) exists and produces valid DocumentIndex
- [ ] from_scan produces equivalent heading/link/tag/block_id data as from_ast for clean markdown
- [ ] zig-kernels feature flag in markymark-index/Cargo.toml properly gates the new code
- [ ] Without zig-kernels: cargo test -p markymark-index passes (no changes to existing behavior)
- [ ] With zig-kernels: cargo test -p markymark-index --features zig-kernels passes
- [ ] Parity tests on 10+ test documents show equivalent extraction results
- [ ] cargo clippy -p markymark-index -- -D warnings is clean

## Implementation Checklist
- [ ] Add zig-kernels feature to markymark-index/Cargo.toml: zig-kernels = ["markymark-core/zig-kernels"]
- [ ] Add from_scan method to DocumentIndex (or parallel constructor)
- [ ] Map ScanBackend results to existing DocumentIndex internal types
- [ ] Convert byte offsets from ScanBackend to DocumentIndex's position format
- [ ] Handle the difference: ScanBackend returns raw offsets, tree-sitter returns AST nodes
- [ ] Write 10+ test documents with known structure (headings, links, tags, blocks)
- [ ] Write parity tests: from_ast(parse(text)) == from_scan(text, &zig_backend)
- [ ] Test with code blocks (known false positive source for Zig backend)
- [ ] Measure false positive rate and document it

## Edge Cases
- Empty document: both from_ast and from_scan should produce empty DocumentIndex
- Document with only code blocks: Zig backend may produce false positives, tree-sitter won't
- Document with frontmatter: tree-sitter handles frontmatter, Zig scan may not (document gap)
- Very large document (>100KB): both paths must handle without performance regression
- Document with no structural elements: both paths produce empty extraction results
- Mixed heading styles (ATX # and setext underline): Zig only detects ATX, tree-sitter detects both
- Offset format mismatch: ScanBackend uses byte offsets, DocumentIndex may use line:col

## Anti-patterns
- NO duplicating DocumentIndex construction logic (share builder between from_ast and from_scan)
- NO making from_scan the default without benchmarking (tree-sitter remains default)
- NO ignoring known differences (setext headings, frontmatter) — document them as known gaps
- NO testing only happy path (must test documents with code blocks to measure false positives)
- NO breaking the existing from_ast path (must remain unchanged and working)

## Error Handling
- ScanBackend returns error: propagate as DocumentIndex construction error
- Offset out of bounds: clamp to document length, log warning
- UTF-8 boundary issue in offset: round to nearest valid char boundary
- Feature flag off: from_scan method does not exist (compile error if called)

## Test Specifications (what bug does each test catch?)
- test_from_scan_empty_document: catches null handling in scan-based construction
- test_from_scan_single_heading: catches basic offset-to-position conversion error
- test_from_scan_multiple_headings: catches heading ordering or level confusion
- test_from_scan_markdown_links: catches link offset calculation in from_scan path
- test_from_scan_wiki_links: catches wiki-link type not being preserved in DocumentIndex
- test_from_scan_tags: catches tag offset extraction in from_scan path
- test_from_scan_block_ids: catches block ID extraction in from_scan path
- test_parity_simple_doc: catches systematic difference between from_ast and from_scan
- test_parity_complex_doc: catches edge case differences on real-world document
- test_parity_code_blocks: documents and measures false positive rate from code blocks
- test_parity_10_test_docs: catches regression across diverse document structures
- test_feature_flag_gate: catches from_scan being available without zig-kernels feature
- test_from_ast_unchanged: catches accidental regression in existing from_ast path
