# BRZA Markymark Benchmarks

- Date: 2026-02-17
- Command: `MARKYMARK_BENCH_SAMPLE_SIZE=10 cargo bench -p markymark-kernels --bench brza_kernels -- --output-format bencher`
- Criterion source: `target/criterion/**/new/estimates.json` (mean point estimates)

## Hardware

- OS: `Darwin 25.2.0` (arm64)
- CPU: `Apple M2 Max` (12 cores)
- Memory: `32 GiB`
- Rust: `rustc 1.93.1`, `cargo 1.93.1`

## Heading Scan vs Tree-Sitter

| Corpus | Zig heading_scan | Tree-sitter heading extraction | Speedup (Tree/Zig) | Target 10-100x |
|---|---:|---:|---:|---:|
| 1KB | 2.816 us | 566.849 us | 201.27x | Exceeds |
| 10KB | 22.189 us | 5.666 ms | 255.37x | Exceeds |
| 100KB | 472.825 us | 50.614 ms | 107.05x | Exceeds |

## Link Scan vs Regex

| Corpus | Zig link_scan | Regex links baseline | Speedup (Regex/Zig) | Target 10-50x |
|---|---:|---:|---:|---:|
| 100 links | 171.638 us | 14.445 us | 0.08x | Miss |
| 500 links | 422.492 us | 64.128 us | 0.15x | Miss |
| 2K links | 422.412 us | 306.191 us | 0.72x | Miss |

## Embedding Search (Top-10)

| Index size | Mean latency | Target |
|---|---:|---:|
| 1K | 0.300 ms | <1 ms (Pass) |
| 10K | 4.260 ms | <5 ms (Pass) |
| 100K | 41.054 ms | <10 ms (Miss) |

## Content Hash vs MD5

| Corpus | Zig content_hash (FNV-1a) | md5 baseline | Speedup (MD5/Zig) | Target 2-5x |
|---|---:|---:|---:|---:|
| 1KB | 3.430 us | 2.195 us | 0.64x | Miss |
| 10KB | 30.761 us | 22.486 us | 0.73x | Miss |
| 100KB | 356.270 us | 195.857 us | 0.55x | Miss |

## Bulk Re-index (600 docs)

| Mode | Mean time | Speedup (Tree/Zig) |
|---|---:|---:|
| Zig scan backend (`DocumentIndex::from_scan`) | 25.124 ms | 40.03x |
| Tree-sitter parse + `DocumentIndex::from_ast` | 1005.579 ms | baseline |

## Summary

- Strong wins: heading extraction and bulk re-indexing (100x+ and 40x).
- Meets latency target: embedding search at 1K and 10K.
- Misses: embedding 100K target, link scan vs regex target, and content hash vs md5 target.
- Outcome: benchmark harness and report are complete; optimization follow-ups are needed for missed BRZA targets.
