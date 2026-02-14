# Pre-Arena Baseline Benchmarks

**Branch:** `baseline/pre-arena`  
**Commit:** `476795e` — chore: checkpoint — marky-cfj complete, v0.1.0-alpha.2 released (#5)  
**Parent of:** `47aada5` (arena Phase 1)  
**Captured:** 2026-02-14

## Reproducing

```bash
git checkout baseline/pre-arena
cargo bench -p markymark-index -- --nocapture
```

## Results (synthetic sample: 100 docs)

| Benchmark                  | Mean     | Unit   |
|---------------------------|----------|--------|
| index_10_documents        | 3.32     | ms     |
| index_100_documents       | 33.84    | ms     |
| reparse_single_document   | 325.53   | µs     |
| memory/memory_after_index_100 | 33.45 | ms     |
| memory/alloc_count_index_100  | 33.65 | ms     |
| concurrent_index_4x100_docs   | 59.09 | ms     |
| concurrent_index_8x100_docs   | 103.92| ms     |

### Key metrics (from run output)

| Metric                | Value        |
|-----------------------|--------------|
| Heap allocations (100 docs) | 216,806  |
| Resident memory       | 23 MiB       |
| Peak RSS              | 23,680 KB    |

## Arena Comparison (feature/feature-001)

| Metric              | Baseline (476795e) | Arena (current) | Delta   |
|---------------------|-------------------|-----------------|---------|
| Heap allocations    | 216,806           | 215,837         | −969    |
| index_100           | ~33.8 ms          | ~33.3 ms        | ~same   |
| concurrent 4×100    | ~59 ms            | ~59 ms          | ~same   |
| concurrent 8×100    | ~104 ms           | ~101 ms         | −3%     |
