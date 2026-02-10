## Performance — Zero-Cost Abstractions & Profiling

> **TL;DR:** Rust's abstractions (iterators, generics, traits) compile to the same code
> you'd write by hand. Profile before optimizing. Identify hot paths early, use
> `#[inline]` judiciously, and switch apps to mimalloc.

### Zero-Cost Abstractions

Rust's core promise: **abstractions have no runtime overhead** compared to hand-written code.

```rust
// This iterator chain:
let sum: i32 = data.iter().filter(|x| **x > 0).map(|x| x * 2).sum();

// Compiles to the same assembly as:
let mut sum = 0i32;
for x in data {
    if *x > 0 { sum += x * 2; }
}
```

### Profiling Workflow

1. **Benchmark first** — use `criterion` or `divan`
2. **Profile** — use `perf`, `Instruments`, `cargo-flamegraph`
3. **Identify hot paths** — focus on the top contributors
4. **Optimize** — apply targeted changes
5. **Re-benchmark** — verify improvement

```toml
# Enable debug symbols in release for profiling
[profile.release]
debug = 1

[profile.bench]
debug = 1
```

### Benchmarking with criterion

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_sort(c: &mut Criterion) {
    c.bench_function("sort 1000 elements", |b| {
        b.iter(|| {
            let mut data = black_box(vec![3, 1, 4, 1, 5, 9; 1000]);
            data.sort();
        })
    });
}

criterion_group!(benches, bench_sort);
criterion_main!(benches);
```

### Performance Patterns

| Pattern | Effect |
|---------|--------|
| Use iterators over indexed loops | Better vectorization, no bounds checks |
| `#[inline]` on small hot functions | Enables cross-crate inlining |
| `#[inline(never)]` on cold paths | Keeps hot code compact |
| `Vec::with_capacity(n)` | Avoids reallocations |
| `String::with_capacity(n)` | Avoids reallocations |
| `HashMap` with `ahash` | Faster hashing (non-crypto) |
| `mimalloc` global allocator | 10-25% improvement for allocation-heavy code |
| `Cow<str>` | Avoids allocation when borrowing suffices |
| `SmallVec` | Stack-allocates small collections |

### #[inline] Guidelines

```rust
// ✅ Inline: small, frequently called, crosses crate boundaries
#[inline]
pub fn is_valid(&self) -> bool { self.value > 0 }

// ✅ Always inline: critical hot path, very small
#[inline(always)]
pub fn get_flag(&self) -> bool { self.flags & 0x01 != 0 }

// ✅ Never inline: error paths, cold branches
#[inline(never)]
fn handle_error(e: &Error) { /* ... */ }

// ❌ Don't inline: large functions, rarely called
```

### mimalloc for Applications

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

### Async Yield Points

Long-running CPU-bound async tasks should yield to avoid starving other tasks:

```rust
async fn process_batch(items: Vec<Item>) {
    for (i, item) in items.iter().enumerate() {
        process_item(item);
        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }
}
```

### References

- Guidelines: [M-HOTPATH](../../docs/rust_guidelines/performance.md), [M-THROUGHPUT](../../docs/rust_guidelines/performance.md), [M-YIELD-POINTS](../../docs/rust_guidelines/performance.md)
- Guidelines: [M-MIMALLOC-APPS](../../docs/rust_guidelines/applications.md)
- criterion: [docs.rs/criterion](https://docs.rs/criterion/)
- Related: [checklists/performance.md](../checklists/performance.md)
