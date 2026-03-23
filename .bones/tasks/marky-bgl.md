---
id: marky-bgl
title: Port similarity and entity hash kernels from forge BRZA
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv, marky-een]
---




Fork from forge BRZA: similarity.zig (cosine + jaccard), entities.zig (FNV-1a entity extraction), quantize.zig (Q4 quant/dequant), normalize.zig (L2). Place in zig/src/shared/. Add C ABI exports: zig_cosine_similarity, zig_jaccard_similarity, zig_extract_entity_hashes, asm_normalize_f32_l2, asm_quantize_f32_to_q4_0, asm_dequantize_q4_0_to_f32. Also port scalar reference implementations to src/reference/. Write Zig unit tests: correctness vs reference, SIMD vs scalar parity. See brza-spec.md Sections 3.1-3.2 for API.

## Design

## Goal
Fork from forge BRZA: similarity.zig (cosine + jaccard), entities.zig (FNV-1a entity extraction), quantize.zig (Q4 quant/dequant), normalize.zig (L2 normalization). Place in zig/src/shared/. Add all C ABI exports. Port scalar reference implementations for parity testing. Write comprehensive Zig unit tests for correctness vs reference and SIMD vs scalar parity.

## Effort Estimate
8-10 hours

## Success Criteria
- [ ] zig/src/shared/similarity.zig: cosine_similarity and jaccard_similarity pass all tests
- [ ] zig/src/shared/entities.zig: FNV-1a entity extraction matches reference implementation
- [ ] zig/src/shared/quantize.zig: Q4 round-trip error < 0.1 for typical embedding values
- [ ] zig/src/shared/normalize.zig: L2 normalization produces unit vectors (norm within 1e-6 of 1.0)
- [ ] Scalar reference implementations in src/reference/ for each kernel
- [ ] SIMD vs scalar parity: identical results for all test inputs
- [ ] All C ABI exports callable: zig_cosine_similarity, zig_jaccard_similarity, zig_extract_entity_hashes, asm_normalize_f32_l2, asm_quantize_f32_to_q4_0, asm_dequantize_q4_0_to_f32
- [ ] cd zig && zig build test passes all tests

## Implementation Checklist
- [ ] Port zig/src/shared/similarity.zig (update for Zig 0.15.2)
- [ ] Port zig/src/shared/entities.zig (update for Zig 0.15.2)
- [ ] Port zig/src/shared/quantize.zig (update for Zig 0.15.2)
- [ ] Port zig/src/shared/normalize.zig (update for Zig 0.15.2)
- [ ] Create zig/src/reference/similarity_ref.zig (scalar implementations)
- [ ] Create zig/src/reference/entities_ref.zig
- [ ] Create zig/src/reference/quantize_ref.zig
- [ ] Create zig/src/reference/normalize_ref.zig
- [ ] Add C ABI exports to c_adapter.zig for all 6 functions
- [ ] Write parity tests (SIMD output == scalar reference output for same input)
- [ ] Write edge case tests per kernel
- [ ] Update build.zig to compile new files

## Edge Cases
- Empty input vectors: cosine/jaccard should handle len=0 (return 0.0 or -1)
- Identical vectors: cosine should return 1.0, jaccard should return 1.0
- Orthogonal vectors: cosine should return 0.0
- Zero vector: L2 normalize should handle gracefully (return -1, not NaN/Inf)
- Very large entity text (>1MB): entity extraction should not overflow internal buffers
- Q4 quantization of values outside [-1, 1]: should clamp, not produce garbage
- Unicode text in entity extraction: FNV-1a hashes bytes, so UTF-8 is handled naturally
- Jaccard with disjoint sets: should return 0.0
- Jaccard at large N (1000+ hashes): BE CAUTIOUS — forge implementation FAILED perf gates here

## Anti-patterns
- NO batch Jaccard optimization without profiling first (forge lesson: failed at 1000+ episodes)
- NO Zig 0.14.x @Vector patterns (check 0.15.2 langref for @Vector changes)
- NO heap allocation for caller buffers (Rust allocates, Zig writes)
- NO FNV-1a implementation that differs from reference (hash values must be identical)
- NO skipping SIMD-vs-scalar parity tests (they catch subtle bit-level differences)
- NO tests that only check "compiles" without verifying numerical correctness

## Error Handling
- Null pointer on any input: return -1
- Zero length on vectors: return -1 (cosine/jaccard undefined for empty sets)
- Zero vector for normalize: return -1 (cannot normalize zero vector)
- Buffer too small for entity hashes: return -2, write as many as fit
- Q4 quantize with n not divisible by block size: return -1 or pad

## Test Specifications (what bug does each test catch?)
- test_cosine_identical: catches sign/magnitude errors in dot product (must return 1.0)
- test_cosine_orthogonal: catches missing normalization (would return 0 only if properly normalized)
- test_cosine_opposite: catches incorrect handling of negative values (must return -1.0)
- test_jaccard_disjoint: catches union calculation bug (should be 0.0)
- test_jaccard_identical: catches intersection/union confusion (should be 1.0)
- test_jaccard_partial_overlap: catches off-by-one in set intersection counting
- test_entity_hash_determinism: catches use of random seed or uninitialized state
- test_entity_hash_known_values: catches FNV-1a algorithm implementation error
- test_normalize_unit_length: catches missing sqrt or incorrect accumulation in L2 norm
- test_normalize_zero_vector: catches NaN propagation from divide-by-zero
- test_quantize_round_trip: catches information loss beyond acceptable threshold
- test_quantize_clamp_range: catches overflow when input exceeds expected range
- test_simd_scalar_parity_cosine: catches SIMD lane ordering or accumulation rounding differences
- test_simd_scalar_parity_entities: catches SIMD boundary misalignment in text scanning
