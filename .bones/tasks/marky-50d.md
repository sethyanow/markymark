---
id: marky-50d
title: Implement tag_scan and block_scan Zig SIMD kernels
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
---




Create zig/src/kernels/tag_scan.zig and block_scan.zig. tag_scan: SIMD scan for #tag patterns (# followed by alphanumeric/hyphen, preceded by whitespace or line start). block_scan: SIMD scan for ^block-id patterns (^ followed by alphanumeric/hyphen at end of line). Structs per brza-markymark.md Section 4.2. Export as marky_scan_tags and marky_scan_block_ids. Tests for each kernel.

## Design

## Goal
Create zig/src/kernels/tag_scan.zig and block_scan.zig. tag_scan: SIMD scan for #tag patterns (# followed by alphanumeric/hyphen, preceded by whitespace or line start). block_scan: SIMD scan for ^block-id patterns (^ followed by alphanumeric/hyphen at end of line). Both are simpler pattern kernels that share similar SIMD strategies.

## Effort Estimate
6-8 hours (two kernels, but simpler patterns than heading/link)

## Success Criteria
- [ ] tag_scan.zig compiles and exports marky_scan_tags via c_adapter.zig
- [ ] block_scan.zig compiles and exports marky_scan_block_ids via c_adapter.zig
- [ ] TagScan struct matches spec: offset: u32, length: u16, _padding: [2]u8
- [ ] BlockIdScan struct matches spec: offset: u32, length: u16, _padding: [2]u8
- [ ] Tags: detects #tag but not email@#tag or mid-word#tag (whitespace/line-start boundary)
- [ ] Block IDs: detects ^block-id only at end of line (before \n or EOF)
- [ ] Scalar reference implementations for both kernels
- [ ] SIMD vs scalar parity on all test inputs
- [ ] cd zig && zig build test passes

## Implementation Checklist
- [ ] Create zig/src/kernels/tag_scan.zig
- [ ] Create zig/src/kernels/block_scan.zig
- [ ] Create zig/src/reference/tag_scan_ref.zig
- [ ] Create zig/src/reference/block_scan_ref.zig
- [ ] Define TagScan and BlockIdScan extern structs
- [ ] tag_scan SIMD: scan for '#' characters, check preceding char is whitespace/SOL
- [ ] tag_scan: extract tag name (alphanumeric + hyphen + underscore characters after #)
- [ ] block_scan SIMD: scan for '^' characters, verify at end of line
- [ ] block_scan: extract block ID (alphanumeric + hyphen after ^)
- [ ] Add C ABI exports to c_adapter.zig
- [ ] Write unit tests for both kernels
- [ ] Write SIMD vs scalar parity tests
- [ ] Update build.zig

## Edge Cases
- Empty input: return 0 written, status 0
- Tag at line start: #tag at position 0 should be detected
- Tag after space: "text #tag" should detect #tag
- Tag in mid-word: "word#tag" should NOT be detected (not a tag boundary)
- Tag with only digits: #123 — decide if valid (Obsidian: yes, some systems: no). Default: accept.
- Tag with underscore: #my_tag — should be detected
- Hash heading vs tag: "# heading" is a heading not a tag (space after # distinguishes)
- Block ID not at EOL: "^id more text" — should NOT be detected (only at line end)
- Block ID at EOF: "text ^id" (no trailing newline) — should be detected
- Multiple tags on one line: "text #tag1 #tag2" — detect both
- Unicode in tag name: #cafe-au-lait — ASCII chars only in name (non-ASCII terminates)
- Buffer full: return -2

## Anti-patterns
- NO confusing heading detection with tag detection (# heading vs #tag — space is the key)
- NO detecting tags inside code spans (`#not-a-tag`)
- NO heap allocation for intermediate scanning state
- NO O(n^2) scanning for end-of-line in block_scan (SIMD scan for '\n' first)
- NO tests that only verify count without checking offsets

## Error Handling
- Null text pointer: return -1
- Zero length: return 0 written, status 0
- Null output/written pointers: return -1
- Buffer too small: return -2, write as many results as fit

## Test Specifications (what bug does each test catch?)
- test_tag_empty_input: catches null deref on zero-length text
- test_tag_at_line_start: catches missing SOL boundary check (only checking after space)
- test_tag_after_space: catches basic tag detection and offset calculation
- test_tag_in_mid_word: catches false positive for non-boundary hash characters
- test_tag_heading_distinction: catches confusing "#heading" with "#tag" (space matters)
- test_tag_with_hyphen_underscore: catches premature tag name termination on valid chars
- test_tag_multiple_per_line: catches state machine not resetting between tags
- test_block_id_at_eol: catches basic block ID detection
- test_block_id_not_at_eol: catches false positive for ^id in middle of line
- test_block_id_at_eof: catches off-by-one at end of input without newline
- test_block_id_with_preceding_text: catches incorrect offset for "text ^id\n"
- test_tag_buffer_overflow: catches writing past cap in tag scan
- test_block_buffer_overflow: catches writing past cap in block scan
- test_simd_scalar_parity_tags: catches SIMD boundary alignment bugs in tag scan
- test_simd_scalar_parity_blocks: catches SIMD boundary alignment bugs in block scan
