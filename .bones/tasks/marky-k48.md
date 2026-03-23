---
id: marky-k48
title: SIMD scanning engine (vectorized state transitions, match emission)
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-83b]
parent: marky-8s3.2
---




## Design

## Goal
Implement the SIMD-accelerated scan loop that drives the Aho-Corasick automaton. Vectorize byte comparison for common fast-path (no pattern prefix bytes present in 16-byte chunk = skip entire chunk). On match candidates, fall back to scalar automaton transition. Emit ScanResult entries with type discriminator and byte offsets.

## Effort Estimate
5-6 hours

## Success Criteria
- [ ] SIMD fast path: skip 16-byte chunks with no pattern prefix bytes
- [ ] Scalar fallback for chunks containing pattern prefix bytes
- [ ] ScanResult struct: { type: u8, offset: u32, length: u16, extra: u16 }
- [ ] Results sorted by byte offset
- [ ] Handles documents up to 1MB without stack overflow
- [ ] SIMD vs scalar reference parity test passes
- [ ] Performance: >= 1.5x faster than 4 separate scan kernel calls

## Edge Cases
- Pattern at SIMD vector boundary (spanning two 16-byte chunks)
- Document shorter than SIMD vector width (pure scalar fallback)
- Very long document with many matches (buffer management)

## Anti-patterns
- NO byte-by-byte scanning in SIMD path (defeats purpose)
- NO heap allocation per chunk processed
- NO returning results in non-deterministic order

## Test Specifications
- test_simd_scalar_parity: catches SIMD boundary bugs
- test_short_document: catches crash on input < 16 bytes
- test_many_matches: catches buffer management issues
- test_performance_vs_separate: catches regression vs individual kernels
