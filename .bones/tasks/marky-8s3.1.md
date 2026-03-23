---
id: marky-8s3.1
title: Implement code fence exclusion map Zig kernel
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3
---





Create zig/src/kernels/fence_map.zig. SIMD scan for fenced code block boundaries (triple backtick and triple tilde). Produces a bitmap/range list of byte ranges that are inside code blocks. All other scan kernels (heading, link, tag, block) consult this map to eliminate false positives. Export as marky_build_fence_map(text, len, ranges_out, cap, written) -> i32. FenceRange struct: { start: u32, end: u32 }. Tests: no fences, single fence, nested fences (should not happen in valid MD but handle gracefully), fence at EOF, fence with language specifier. This is the foundational kernel that enables the promotion path (brza-markymark.md Section 5.2).

## Design

## Goal
Create zig/src/kernels/fence_map.zig implementing SIMD scan for fenced code block boundaries (triple backtick and triple tilde). Produces a range list of byte ranges inside code blocks. This is the foundational kernel enabling the promotion path — all other scan kernels consult this map to eliminate false positives from code blocks.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] fence_map.zig compiles and exports marky_build_fence_map via c_adapter.zig
- [ ] FenceRange struct: { start: u32, end: u32 } (8 bytes, C ABI compatible)
- [ ] Correctly identifies triple backtick fences (opening and closing)
- [ ] Correctly identifies triple tilde fences (opening and closing)
- [ ] Handles fence with language specifier: ```python
- [ ] Handles fence at EOF without closing (treat rest of document as fenced)
- [ ] Scalar reference implementation for parity testing
- [ ] SIMD vs scalar parity on all test inputs
- [ ] cd zig && zig build test passes

## Implementation Checklist
- [ ] Create zig/src/kernels/fence_map.zig
- [ ] Create zig/src/reference/fence_map_ref.zig (scalar implementation)
- [ ] Define FenceRange extern struct
- [ ] SIMD phase: scan for '`' and '~' characters using @Vector comparison
- [ ] When triple backtick/tilde found at line start, toggle fence state
- [ ] Track open/close pairs, recording byte ranges
- [ ] Handle unclosed fence at EOF: range extends to end of input
- [ ] Handle language specifier after opening fence (skip to newline)
- [ ] Handle 4+ backticks (valid markdown, different nesting level)
- [ ] Add C ABI export to c_adapter.zig
- [ ] Write unit tests
- [ ] Update build.zig

## Edge Cases
- Empty input: return 0 ranges, status 0
- No fences: return 0 ranges, status 0
- Single unclosed fence: one range from fence start to EOF
- Nested fences (4 backticks inside 3): technically valid in CommonMark, handle gracefully
- Fence at line start only (indented backticks are not fences in CommonMark)
- Tilde fences (~~~): same behavior as backtick fences
- Mixed fence types: ``` opened by backtick, cannot be closed by ~~~
- Inline code (single backtick): must NOT trigger fence detection
- Fence with trailing spaces: ``` \n should still be a fence
- Adjacent fences: ```\n```\n — empty code block, produces one range
- Buffer full: return -2, write as many ranges as fit

## Anti-patterns
- NO confusing inline code (single backtick) with fenced code blocks (triple backtick)
- NO assuming fences are always backticks (tildes are equally valid)
- NO allowing tilde fence to close backtick fence (they must match)
- NO ignoring indentation rules (indented ``` is not a fence in CommonMark)
- NO heap allocation for the range list (Rust allocates output buffer)
- NO O(n^2) scanning (must be single-pass or two-pass at most)

## Error Handling
- Null text pointer: return -1
- Zero length: return 0 written, status 0
- Null output/written pointers: return -1
- Buffer too small: return -2, write as many complete FenceRange entries as fit

## Test Specifications (what bug does each test catch?)
- test_empty_input: catches null deref on zero-length text
- test_no_fences: catches false positive from inline backticks
- test_single_fence_pair: catches basic fence detection and range calculation
- test_unclosed_fence: catches failure to extend range to EOF
- test_multiple_fences: catches state machine not toggling between fenced/unfenced
- test_tilde_fences: catches only handling backtick fences
- test_mixed_fence_types: catches backtick fence being closed by tilde fence
- test_inline_code_not_fence: catches single backtick triggering fence state
- test_fence_with_language: catches language specifier breaking fence detection
- test_four_backtick_fence: catches hardcoded "exactly 3" instead of "3 or more"
- test_indented_backticks: catches treating indented code as a fence
- test_adjacent_empty_fences: catches off-by-one in range boundaries for empty blocks
- test_buffer_overflow: catches writing past cap boundary
- test_simd_scalar_parity: catches SIMD boundary alignment bugs
