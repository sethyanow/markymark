---
id: marky-8s3.2
title: Implement single-pass multi-pattern scanner (Aho-Corasick)
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3.1, marky-ccv, marky-v8r, marky-yo7, marky-50d]
parent: marky-8s3
---










Create zig/src/kernels/multi_scan.zig. Single-pass Aho-Corasick automaton that finds ALL markdown structural elements simultaneously: headings (#), wiki-links ([[), markdown links ([), tags (#tag), block IDs (^), and code fences. Uses fence_map output to filter false positives. Returns a unified ScanResult array with type discriminator. Replaces separate heading/link/tag/block scan calls with ONE FFI call. Export as marky_multi_scan(text, len, fence_map, fence_count, results_out, cap, written) -> i32. Tests: mixed document with all element types, performance comparison vs 4 separate scans. Depends on fence_map kernel.

## Design

## Goal
Coordinator: Implement single-pass multi-pattern scanner. Broken into 3 subtasks:
(a) Automaton construction — pattern trie with goto/failure functions
(b) SIMD scanning engine — vectorized state transitions
(c) Result partitioning — fence-map filtering and C ABI export

## Success Criteria
- [ ] All 3 child subtasks closed
- [ ] Parity test: multi_scan matches combined individual kernel results
- [ ] Performance: >= 1.5x faster than 4 separate scan calls
