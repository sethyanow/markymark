---
id: marky-e59
title: 'Benchmark suite: SIMD kernels vs baseline'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0oz, marky-bcv]
---



Write criterion benchmarks for: heading_scan vs tree-sitter heading extraction, link_scan vs regex link extraction, embedding search at 1K/10K/100K entries, content_hash vs md5, bulk re-index 600 docs with Zig vs tree-sitter. Produce markdown report similar to arena ROI report (docs/benchmarks/brza-markymark-benchmarks.md). Target speedups per brza-markymark.md Section 9.3.

## Design

## Goal
Write criterion benchmarks for all BRZA kernels: heading_scan vs tree-sitter, link_scan vs regex, embedding search at 1K/10K/100K entries, content_hash vs md5, and bulk re-index with Zig vs tree-sitter. Produce a markdown report at docs/benchmarks/brza-markymark-benchmarks.md with results and analysis.

## Effort Estimate
8-10 hours

## Success Criteria
- [ ] Criterion benchmark suite in benches/brza_kernels.rs
- [ ] heading_scan vs tree-sitter heading extraction: measured ops/sec, target 10-100x
- [ ] link_scan vs regex link extraction: measured ops/sec, target 10-50x
- [ ] embedding search at 1K, 10K, 100K entries: measured latency, targets <1ms, <5ms, <10ms
- [ ] content_hash vs md5: measured ops/sec, target 2-5x
- [ ] bulk re-index (project docs): total time with Zig vs tree-sitter
- [ ] Benchmark report at docs/benchmarks/brza-markymark-benchmarks.md
- [ ] All benchmarks run: cargo bench --features zig-kernels
- [ ] Results include both absolute numbers and relative speedup factors

## Implementation Checklist
- [ ] Add criterion to dev-dependencies in workspace Cargo.toml
- [ ] Create benches/brza_kernels.rs with criterion benchmark groups
- [ ] Benchmark: heading_scan (Zig) on 1KB, 10KB, 100KB markdown documents
- [ ] Benchmark: tree-sitter heading extraction on same documents (baseline)
- [ ] Benchmark: link_scan (Zig) on link-heavy documents
- [ ] Benchmark: regex link extraction on same documents (baseline)
- [ ] Benchmark: embedding index search at 1K, 10K, 100K entries (random embeddings)
- [ ] Benchmark: content_hash (FNV-1a) on various document sizes
- [ ] Benchmark: md5 hash on same documents (baseline)
- [ ] Benchmark: bulk re-index project docs/ with Zig backend vs tree-sitter
- [ ] Generate markdown report with tables and analysis
- [ ] Include hardware info in report (CPU, memory, OS)

## Edge Cases
- Small documents (<1KB): Zig SIMD overhead may make it slower than scalar (measure!)
- Very large documents (>1MB): SIMD should shine here, but verify
- Cold vs warm: first run includes cache misses, warm runs are more representative
- Criterion sample size: ensure sufficient iterations for statistical significance
- Embedding index with duplicate entries: may affect search performance
- Non-representative test data: use realistic markdown, not synthetic repetitive content

## Anti-patterns
- NO using system time for benchmarking (use criterion's statistical framework)
- NO benchmarking in debug mode (must be --release)
- NO comparing across different hardware without documenting the hardware
- NO claiming speedup without statistical confidence intervals
- NO benchmarking only the best case (include worst case: code-block-heavy docs for false positive overhead)
- NO ignoring warm-up and cache effects (criterion handles this, but verify)

## Error Handling
- Benchmark setup failure: skip benchmark with warning, don't abort suite
- Missing feature flag: clear error message about needing --features zig-kernels
- Embedding provider not available: use mock embeddings for search benchmarks
- File not found for corpus benchmarks: log warning, skip file

## Test Specifications (what bug does each test catch?)
- bench_heading_scan_1kb: establishes baseline for small document performance
- bench_heading_scan_10kb: catches SIMD not engaging on medium documents
- bench_heading_scan_100kb: catches O(n^2) algorithms that only show at scale
- bench_heading_scan_vs_tree_sitter: catches SIMD being slower than tree-sitter (fails validation)
- bench_link_scan_vs_regex: catches SIMD being slower than regex baseline
- bench_embedding_search_1k: establishes baseline search latency
- bench_embedding_search_100k: catches O(n) search not meeting <10ms target
- bench_content_hash_vs_md5: catches FNV-1a being slower than md5 (unexpected)
- bench_bulk_reindex: catches end-to-end integration not delivering expected speedup
- bench_small_doc_overhead: catches SIMD setup cost dominating on small inputs
