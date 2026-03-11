---
id: marky-v8r
title: Implement heading_scan Zig SIMD kernel
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
---





Create zig/src/kernels/heading_scan.zig. SIMD scan for '#' at line start followed by space. Extract: byte offset, heading text length, level (1-6). Write scalar reference in src/reference/. HeadingScan struct per brza-markymark.md Section 4.2. Export via c_adapter.zig as marky_scan_headings. Tests: empty text, single heading, multiple levels, consecutive headings, heading at EOF without newline. Benchmark: SIMD vs scalar on 10KB markdown.

## Design

## Goal
Create zig/src/kernels/heading_scan.zig implementing SIMD-accelerated heading extraction. Scans for '#' at line start followed by space, extracts byte offset, heading text length, and level (1-6). Includes scalar reference implementation and comprehensive tests. This is the first markymark-specific extraction kernel.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] heading_scan.zig compiles and exports marky_scan_headings via c_adapter.zig
- [ ] Scalar reference implementation in src/reference/heading_scan_ref.zig
- [ ] SIMD vs scalar parity: identical results for all test inputs
- [ ] Correctly extracts level 1-6 headings with byte offset and text length
- [ ] Ignores '#' not at line start (e.g., mid-line hash characters)
- [ ] Benchmark: SIMD version >= 2x faster than scalar on 10KB markdown
- [ ] All Zig tests pass: cd zig && zig build test
- [ ] HeadingScan struct matches spec Section 4.2 (offset: u32, length: u16, level: u8, _padding: u8)

## Implementation Checklist
- [ ] Create zig/src/kernels/heading_scan.zig with SIMD implementation
- [ ] Create zig/src/reference/heading_scan_ref.zig with scalar loop implementation
- [ ] Add marky_scan_headings export to c_adapter.zig
- [ ] Define HeadingScan extern struct matching spec
- [ ] Implement SIMD: scan 16 bytes at a time for '\n' boundaries, check next char for '#'
- [ ] Handle first-line heading (no preceding newline)
- [ ] Count consecutive '#' chars for level (cap at 6)
- [ ] Find heading text: skip '#'s and space, scan to newline or EOF
- [ ] Write unit tests (see test specifications below)
- [ ] Write benchmark: SIMD vs scalar on 10KB sample document
- [ ] Update build.zig to include heading_scan.zig

## Edge Cases
- Empty input (len=0): return 0 written, status 0
- Heading at byte 0 (first line): must detect without preceding newline
- Heading at EOF without trailing newline: must still be detected
- Level > 6 (e.g., "####### text"): should be ignored (not a valid heading) or capped at level 6
- ATX closing hashes ("## Heading ##"): extract text between opening and closing hashes
- Heading with no text ("## "): should produce length=0 heading
- Binary content / non-UTF8: must not crash, may produce garbage results (that is OK)
- Output buffer full: return -2 and write up to cap results
- Very long heading text (>65535 bytes): length field is u16, must cap at u16::MAX

## Anti-patterns
- NO scanning byte-by-byte in the hot path (defeats SIMD purpose)
- NO heap allocation in the kernel (Rust allocates output buffer)
- NO Zig 0.14.x @Vector patterns
- NO ignoring the first line (common bug: only checking after '\n')
- NO tests that only check count without verifying offsets and levels

## Error Handling
- Null text pointer: return -1
- Zero length: return 0 written, status 0 (empty input is valid, just has no headings)
- Null output pointer: return -1
- Zero cap: return -2 (buffer too small, even for 0 would be misleading)
- Null written pointer: return -1

## Test Specifications (what bug does each test catch?)
- test_empty_input: catches null deref or divide-by-zero on zero-length text
- test_single_h1: catches basic scanning failure and offset calculation error
- test_all_levels: catches level parsing bug (must distinguish # through ######)
- test_heading_first_line: catches missing first-line detection (only scanning after '\n')
- test_heading_at_eof: catches off-by-one at end of input (no trailing newline)
- test_consecutive_headings: catches state not resetting between headings
- test_hash_in_middle_of_line: catches false positive for inline hash characters
- test_heading_in_code_block: documents known false positive (code block context unaware)
- test_atx_closing_hashes: catches incorrect text length when closing hashes present
- test_buffer_overflow: catches writing past cap boundary
- test_simd_scalar_parity: catches SIMD lane boundary bugs at 16/32 byte alignment points
- test_level_seven_ignored: catches accepting invalid heading levels > 6
- test_heading_no_space: catches "##no-space" being incorrectly detected (# must be followed by space)
