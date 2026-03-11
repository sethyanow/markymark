---
id: marky-ams
title: Implement ZigScanBackend wrapping markymark-kernels FFI
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8ri, marky-0u5, marky-h1t]
parent: marky-qv6
---





## Design

## Goal
Implement ZigScanBackend struct behind #[cfg(feature = "zig-kernels")]. Zero-size struct that delegates to markymark-kernels FFI wrappers. Also implement ZigEmbeddingIndex wrapping the kernels EmbeddingIndex with proper lifecycle management.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] ZigScanBackend implements ScanBackend using markymark-kernels::scan::*
- [ ] ZigEmbeddingIndex wraps EmbeddingIndex with Mutex for thread safety
- [ ] All methods map KernelError to ScanError/EmbedError correctly
- [ ] Feature-gated: only compiles with zig-kernels feature
- [ ] cargo test -p markymark-core --features zig-kernels passes
- [ ] SAFETY comments on all unsafe blocks

## Edge Cases
- ZigEmbeddingIndex is NOT Send/Sync natively — needs Mutex wrapper
- Empty input: delegates to kernel, kernel returns empty results
- Kernel returns -1/-2/-3: maps to correct ScanError variant

## Anti-patterns
- NO implementing Send/Sync on raw Zig pointer without Mutex
- NO making ZigScanBackend Clone (it's a zero-size struct, but ZigEmbeddingIndex is not clonable)

## Test Specifications
- test_zig_scan_backend_headings: catches FFI integration failure
- test_zig_embedding_lifecycle: catches create/add/search/drop sequence bugs
- test_feature_flag_gate: catches code compiling without zig-kernels feature
