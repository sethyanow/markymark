## Performance Review Checklist

> **TL;DR:** Use this checklist when optimizing or reviewing performance-critical code.

### Allocation & Memory
- [ ] No unnecessary allocations in hot paths
- [ ] `Vec::with_capacity()` / `String::with_capacity()` used when size is known
- [ ] `Cow<str>` used where borrowing is common but ownership sometimes needed
- [ ] Clone not used as escape hatch (restructure borrows instead)
- [ ] No unnecessary `Box<dyn Trait>` when generics would work
- [ ] Application uses `mimalloc` or similar allocator

### Iteration & Computation
- [ ] Iterators used instead of indexed loops where possible
- [ ] `collect()` type hints used to avoid temporary collections
- [ ] Short-circuit evaluation used (`.any()`, `.all()`, `.find()`)
- [ ] Unnecessary copies avoided (pass by reference, use slices)
- [ ] No redundant computations (cache results, avoid re-parsing)

### Concurrency
- [ ] Atomic ordering is appropriate (not over-using SeqCst or under-using with Relaxed)
- [ ] Lock scope minimized (hold lock for shortest time possible)
- [ ] Read-heavy workloads use `RwLock` instead of `Mutex`
- [ ] `Arc::clone()` preferred over cloning inner data
- [ ] No lock contention in hot paths (consider lock-free alternatives)

### Async
- [ ] Async code yields appropriately (`yield_now().await` in CPU-bound loops)
- [ ] `spawn_blocking` used for blocking I/O in async context
- [ ] No `block_on` called inside async context (deadlock risk)
- [ ] Timeouts on all network operations

### Inlining & Codegen
- [ ] `#[inline]` on small, frequently-called functions crossing crate boundaries
- [ ] `#[inline(never)]` on cold/error paths
- [ ] LTO enabled in release profile where appropriate
- [ ] Debug symbols enabled in bench profile for profiler

### Benchmarks & Profiling
- [ ] Benchmarks exist for critical paths (criterion/divan)
- [ ] Profiler run identifies actual hot paths (not guessed)
- [ ] Before/after numbers documented for optimizations
- [ ] No premature optimization (profile first!)

### References
- Detail: [tooling/performance.md](../tooling/performance.md)
- Guidelines: [M-HOTPATH](../../docs/rust_guidelines/performance.md), [M-THROUGHPUT](../../docs/rust_guidelines/performance.md)
