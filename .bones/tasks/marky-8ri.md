---
id: marky-8ri
title: Define ScanBackend and EmbeddingProvider traits in markymark-core
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0u5, marky-h1t]
parent: marky-qv6
---






## Design

## Goal
Create markymark-core/src/scanner.rs with ScanBackend trait and markymark-core/src/embeddings.rs with EmbeddingProvider trait. Define all scan result types (HeadingResult, LinkResult, TagResult, BlockIdResult). Add zig-kernels feature flag to markymark-core/Cargo.toml. Traits must be object-safe (dyn-compatible), Send + Sync.

## Effort Estimate
3-4 hours

## Success Criteria
- [ ] ScanBackend trait: scan_headings, scan_links, scan_tags, scan_block_ids, estimate_tokens
- [ ] EmbeddingProvider trait: embed, embed_batch, dimensions
- [ ] ScanResult types: HeadingResult, LinkResult, TagResult, BlockIdResult with byte offsets
- [ ] ScanError and EmbedError enums with proper variants
- [ ] Both traits are object-safe (Box<dyn ScanBackend> compiles)
- [ ] Both traits are Send + Sync
- [ ] zig-kernels feature flag added to Cargo.toml (no impl yet)
- [ ] cargo test -p markymark-core passes

## Edge Cases
- Trait object safety: no generic methods, no Self-returning methods
- ScanBackend methods take &str (UTF-8 guaranteed by Rust)
- Return types use byte offsets into original text, not owned Strings

## Anti-patterns
- NO async methods in ScanBackend (CPU-bound, async adds complexity)
- NO Zig-specific types in trait interface
- NO making ScanBackend stateful (implementations should be zero-state)

## Test Specifications
- test_scan_backend_trait_object: catches trait not being object-safe
- test_embedding_provider_trait_object: catches EmbeddingProvider not dyn-compatible
- test_scan_backend_send_sync: catches missing Send + Sync bounds
