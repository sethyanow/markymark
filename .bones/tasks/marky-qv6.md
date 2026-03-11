---
id: marky-qv6
title: Add ScanBackend and EmbeddingProvider traits to markymark-core
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0u5, marky-h1t]
---









Create markymark-core/src/scanner.rs with ScanBackend trait (scan_headings, scan_links, scan_tags, scan_block_ids, estimate_tokens methods). Create markymark-core/src/embeddings.rs with EmbeddingProvider trait (embed, embed_batch, dimensions). Add zig-kernels feature flag to markymark-core/Cargo.toml with optional dep on markymark-kernels. Behind zig-kernels flag: implement ZigScanBackend and ZigEmbeddingIndex using markymark-kernels wrappers. Also implement TreeSitterScanBackend that wraps current extraction logic into the ScanBackend trait for parity testing.

## Design

## Goal
Coordinator: Add ScanBackend and EmbeddingProvider traits to markymark-core. Broken into 3 subtasks:
(a) Trait definitions (ScanBackend, EmbeddingProvider, result types) — 3-4h
(b) ZigScanBackend implementation (feature-gated) — 4-6h
(c) TreeSitterScanBackend implementation (always available) — 4-6h

## Success Criteria
- [ ] All 3 child subtasks closed
- [ ] Parity test: ZigScanBackend vs TreeSitterScanBackend on same input
- [ ] Without zig-kernels: cargo test passes (TreeSitter only)
- [ ] With zig-kernels: cargo test --features zig-kernels passes (both backends)
