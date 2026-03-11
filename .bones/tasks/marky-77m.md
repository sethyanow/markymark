---
id: marky-77m
title: Result partitioning and fence-map filtering for multi-scan
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-k48, marky-8s3.1]
parent: marky-8s3.2
---




## Design

## Goal
Post-processing layer: take unified ScanResult array from SIMD engine, filter out results whose offsets fall inside fence_map code block ranges (binary search), and provide C ABI export. Also run type-specific extraction logic (heading level, link URL boundaries, tag text, block ID text) on matches.

## Effort Estimate
4-5 hours

## Success Criteria
- [ ] Binary search filtering against fence_map ranges
- [ ] Type-specific extraction: heading level, link components, tag text, block ID text
- [ ] C ABI export: marky_multi_scan(text, len, fence_map, fence_count, results_out, cap, written)
- [ ] Results filtered + typed match combined output of individual scan kernels
- [ ] Parity test: multi_scan == heading_scan + link_scan + tag_scan + block_scan combined
- [ ] Buffer overflow returns -2 with partial results

## Edge Cases
- Empty fence_map: all results pass through
- All results in code blocks: returns 0 results after filtering
- Fence_map not sorted: must handle or document requirement
- Result at exact fence boundary: define in/out semantics (exclusive end)

## Anti-patterns
- NO O(n*m) filtering (use binary search or sorted merge)
- NO discarding results before filtering (filter after all matches found)

## Test Specifications
- test_fence_filtering_basic: catches results inside code blocks not removed
- test_fence_filtering_all_filtered: catches empty result set handling
- test_parity_with_individual_scans: catches extraction logic divergence
- test_buffer_overflow: catches writing past cap boundary
