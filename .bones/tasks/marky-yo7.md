---
id: marky-yo7
title: Implement link_scan Zig SIMD kernel
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
---




Create zig/src/kernels/link_scan.zig. SIMD scan for [text](url) markdown links and [[wiki-link]] wiki-links. LinkScan struct per brza-markymark.md Section 4.2. Must handle: nested brackets in text, escaped brackets, wiki-links with display text [[target|display]]. Export as marky_scan_links. Tests: markdown links, wiki-links, mixed, empty targets, escaped brackets. Note: does NOT handle code block context (see spec Section 5.2).

## Design

## Goal
Create zig/src/kernels/link_scan.zig implementing SIMD-accelerated link extraction. Detects [text](url) markdown links and [[wiki-link]] wiki-links. Handles nested brackets, escaped brackets, and wiki-links with display text [[target|display]]. This is the most complex extraction kernel due to bracket nesting.

## Effort Estimate
10-12 hours

## Success Criteria
- [ ] link_scan.zig compiles and exports marky_scan_links via c_adapter.zig
- [ ] Correctly extracts markdown links: text offset/length, target offset/length, type=0
- [ ] Correctly extracts wiki-links: target and optional display text, type=1
- [ ] Handles nested brackets in link text (e.g., [text [with] brackets](url))
- [ ] Handles escaped brackets (e.g., \[not a link\])
- [ ] LinkScan struct matches spec Section 4.2
- [ ] Scalar reference implementation for parity testing
- [ ] All Zig tests pass: cd zig && zig build test
- [ ] SIMD vs scalar parity on all test inputs

## Implementation Checklist
- [ ] Create zig/src/kernels/link_scan.zig with SIMD implementation
- [ ] Create zig/src/reference/link_scan_ref.zig with scalar implementation
- [ ] Define LinkScan extern struct matching spec (16 bytes with padding)
- [ ] SIMD phase 1: scan for '[' characters using @Vector comparison
- [ ] State machine: track bracket depth for nested brackets
- [ ] Distinguish [text](url) from [[wiki-link]] by checking second char
- [ ] For markdown links: find matching ']', then '(', scan to matching ')'
- [ ] For wiki-links: find matching ']]', split on '|' for display text
- [ ] Handle escaped brackets: check preceding backslash
- [ ] Add marky_scan_links export to c_adapter.zig
- [ ] Write unit tests (see test specifications)
- [ ] Write SIMD vs scalar parity tests
- [ ] Update build.zig

## Edge Cases
- Empty input (len=0): return 0 written, status 0
- Nested brackets: [text [inner] more](url) — text includes inner brackets
- Escaped bracket: \[not a link\] — must skip
- Empty link text: [](url) — valid, text_length=0
- Empty target: [text]() — valid, target_length=0
- Wiki-link with pipe: [[target|display text]] — must parse both parts
- Wiki-link without pipe: [[simple]] — target and display are the same
- Unclosed bracket: [text without closing — must not hang or consume rest of input
- Adjacent links: [a](b)[c](d) — must detect both
- Image links: ![alt](url) — should detect (link_type could distinguish)
- Very long URL (>65535 bytes): target_length is u16, must cap
- Buffer full: return -2 and write up to cap
- URL with parentheses: [text](url(with)parens) — tricky nested parens in URL

## Anti-patterns
- NO unbounded recursion for bracket matching (use iterative depth counter)
- NO assuming single-byte line boundaries (handle \r\n)
- NO heap allocation for intermediate state
- NO ignoring escaped brackets (backslash-bracket is not a link start)
- NO tests that only verify link count without checking offsets and types

## Error Handling
- Null text pointer: return -1
- Zero length: return 0 written, status 0
- Null output pointer: return -1
- Null written pointer: return -1
- Buffer too small: return -2, write as many complete LinkScan results as fit
- Malformed input (unmatched brackets): skip and continue scanning, never error on content

## Test Specifications (what bug does each test catch?)
- test_empty_input: catches null deref on zero-length text
- test_single_markdown_link: catches basic offset calculation for text and target
- test_single_wiki_link: catches [[]] detection and type field assignment
- test_wiki_link_with_pipe: catches pipe splitting in [[target|display]] format
- test_nested_brackets: catches premature bracket match closing on inner bracket
- test_escaped_bracket: catches false positive on \[ sequences
- test_empty_text: catches crash on [](url) where text_length=0
- test_empty_target: catches crash on [text]() where target_length=0
- test_adjacent_links: catches state machine not resetting between links
- test_unclosed_bracket: catches infinite loop or consuming entire remaining text
- test_url_with_parens: catches premature URL termination on inner parentheses
- test_markdown_and_wiki_mixed: catches type field corruption when alternating link types
- test_buffer_overflow: catches writing past cap boundary
- test_simd_scalar_parity: catches SIMD boundary bugs at alignment edges
- test_image_link: verifies ![alt](url) handling (detected or explicitly skipped)
