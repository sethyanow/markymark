# Pre-Arena Baseline Benchmarks

**Branch:** `baseline/pre-arena`  
**Commit:** `476795e` — chore: checkpoint — marky-cfj complete, v0.1.0-alpha.2 released (#5)  
**Parent of:** `47aada5` (arena Phase 1)  
**Captured:** 2026-02-14

## Reproducing

```bash
git checkout pre-arena              # or baseline/pre-arena (branch with benches)
cargo bench -p markymark-index -- --nocapture
```

### Real corpus (epstein, gigapowers)

When running from a worktree without the epstein fixture:

```bash
MARKYMARK_BENCH_EPSTEIN=/path/to/epstein_20250227_all_in_one.md \
  cargo bench -p markymark-index -- real_corpus --nocapture
```

Gigapowers: set `MARKYMARK_BENCH_CORPUS_DIR` to your local gigapowers checkout path.

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

### Synthetic (100 docs)

| Metric              | Baseline (476795e) | Arena (current) | Delta   |
|---------------------|-------------------|-----------------|---------|
| Heap allocations    | 216,806           | 215,837         | −969    |
| index_100           | ~33.8 ms          | ~33.3 ms        | ~same   |
| concurrent 4×100    | ~59 ms            | ~59 ms          | ~same   |
| concurrent 8×100    | ~104 ms           | ~101 ms         | −3%     |

### Real Corpus (epstein 480 KB, gigapowers 918 files / 5.9 MB) — 2026-02-14

| Benchmark              | Pre-arena | Arena  | Delta    |
|------------------------|-----------|--------|----------|
| reparse_real_large_doc | 166.7 ms  | 163.6 ms | −2%    |
| index_real_corpus (343 sections) | 251.4 ms | 247.8 ms | −1.5% |
| index_docs_dir         | 1.60 s    | 1.64 s   | +2.5% (noise) |

**Verdict:** Real corpus shows no meaningful difference. Arena reduces allocations slightly; latency is within measurement noise.
