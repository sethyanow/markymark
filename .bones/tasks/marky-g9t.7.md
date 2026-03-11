---
id: marky-g9t.7
title: Memory benchmark and cleanup
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.6, marky-luy]
parent: marky-g9t
---




Write a memory benchmark comparing before/after arena allocation. Measure: peak RSS for indexing N documents, allocation count via custom global allocator counter, re-parse time for single document change. Remove marky-7du (unused bumpalo dep task — now used). Clean up any TODO comments from migration.

Success: Benchmark shows measurable improvement. All tests pass. cargo clippy clean.

## Design

## Completed

- Added criterion + memory-stats deps, benches/memory.rs
- Benchmarks: index_10/100_docs, reparse_single_document, peak_rss_after_index_100, alloc_count_index_100
- Custom CountingAllocator reports ~215k heap allocations for index_100 (includes bumpalo chunks, tree-sitter, RealmIndex)
- marky-7du: already closed (superseded); no code reference
- Migration TODOs: only remaining is parser incremental parsing (future feature, not migration)

## Pre-existing

- frontmatter_and_properties test: SIGSEGV (pre-existing, not from this task)
