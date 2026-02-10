## Concurrency — Threads, Atomics & Channels

> **TL;DR:** Rust prevents data races at compile time via `Send`/`Sync`. Share state with
> `Arc<Mutex<T>>`. Default to `Ordering::SeqCst` for atomics — only weaken with proof.
> Use channels for message passing, Rayon for data parallelism.

### Thread Spawning

```rust
use std::thread;

let handle = thread::spawn(|| {
    // This closure runs in a new OS thread
    expensive_computation()
});

// Wait for completion and get result
let result = handle.join().expect("thread panicked");
```

### Send and Sync Traits

| Trait | Meaning | Auto-implemented When |
|-------|---------|----------------------|
| `Send` | Safe to **transfer ownership** to another thread | All fields are `Send` |
| `Sync` | Safe to **share references** between threads | `&T` is `Send` when `T: Sync` |

| Type | Send | Sync | Why |
|------|------|------|-----|
| Primitives, `String`, `Vec` | ✅ | ✅ | No interior mutability |
| `Mutex<T>`, `RwLock<T>` | ✅ | ✅ | Synchronized access |
| `Arc<T>` | ✅ (if `T: Send + Sync`) | ✅ | Atomic reference counting |
| `Rc<T>` | ❌ | ❌ | Non-atomic reference counting |
| `Cell<T>`, `RefCell<T>` | ✅ | ❌ | Unsynchronized interior mutability |
| `*const T`, `*mut T` | ❌ | ❌ | Raw pointers lack guarantees |

### Sharing State

```rust
use std::sync::{Arc, Mutex};
use std::thread;

let counter = Arc::new(Mutex::new(0u64));
let mut handles = vec![];

for _ in 0..10 {
    let counter = Arc::clone(&counter);
    handles.push(thread::spawn(move || {
        let mut num = counter.lock().expect("mutex poisoned");
        *num += 1;
    }));
}

for handle in handles {
    handle.join().unwrap();
}
println!("Count: {}", *counter.lock().unwrap());
```

**Choose:** `Arc<Mutex<T>>` for exclusive access, `Arc<RwLock<T>>` for read-heavy workloads.

### Channels

```rust
use std::sync::mpsc;
use std::thread;

// mpsc: multiple producer, single consumer
let (tx, rx) = mpsc::channel();

let tx2 = tx.clone();  // Multiple producers
thread::spawn(move || { tx.send("from thread 1").unwrap(); });
thread::spawn(move || { tx2.send("from thread 2").unwrap(); });

// Receive
for msg in rx {
    println!("Got: {msg}");
}
```

For multi-consumer or advanced patterns, use `crossbeam::channel`.

### Atomic Ordering Decision Tree

> ⚠️ **COMMON MISTAKE: Defaulting to `Ordering::Relaxed`**
> Relaxed provides NO happens-before guarantees. Use `SeqCst` as the safe default.
> Only downgrade to weaker orderings when you can prove correctness.

```
What are you doing with the atomic?
├─ Simple counter (no synchronization with other data)?
│   └─ Relaxed is OK
│       Example: statistics counter, reference count (with care)
├─ Publishing data (writer side)?
│   └─ Release
│       "All writes before this store are visible to the acquiring thread"
├─ Consuming data (reader side)?
│   └─ Acquire
│       "All writes from the releasing thread are visible after this load"
├─ Read-modify-write (compare_exchange, fetch_add)?
│   └─ AcqRel (Acquire on read, Release on write)
│       Common for lock-free data structures
├─ Need total ordering across ALL threads?
│   └─ SeqCst
│       Strongest guarantee — safe default
└─ Unsure?
    └─ SeqCst — correctness first, optimize later
```

**Ordering summary:**

| Ordering | Guarantees | Use For |
|----------|-----------|---------|
| `Relaxed` | Atomicity only, no ordering | Counters, statistics |
| `Acquire` | Reads after this see Release writes | Lock acquisition, flag checks |
| `Release` | Writes before this visible to Acquire | Lock release, publishing data |
| `AcqRel` | Both Acquire and Release | Compare-and-swap loops |
| `SeqCst` | Total global ordering | Default / when in doubt |

```rust
use std::sync::atomic::{AtomicBool, Ordering};

let flag = AtomicBool::new(false);

// Writer: Release ensures data is visible before flag
flag.store(true, Ordering::Release);

// Reader: Acquire ensures we see data written before flag
if flag.load(Ordering::Acquire) {
    // Safe to read published data
}
```

### Rayon for Data Parallelism

```rust
use rayon::prelude::*;

// Parallel iterator — automatically splits work across threads
let sum: i64 = data.par_iter().map(|x| expensive(x)).sum();

// Parallel sort
let mut items = vec![3, 1, 4, 1, 5, 9];
items.par_sort();
```

### Deadlock Prevention

1. **Always acquire locks in the same order** across all threads
2. **Minimize lock scope** — hold locks for the shortest time possible
3. **Prefer channels** over shared state when possible
4. **Use `try_lock()`** with timeouts for defensive programming
5. **Consider lock-free structures** from `crossbeam` for hot paths

### References

- Nomicon: [Atomics](https://doc.rust-lang.org/nomicon/atomics.html), [Send and Sync](https://doc.rust-lang.org/nomicon/send-and-sync.html)
- The Rust Book: [Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- Guidelines: [M-THROUGHPUT](../../docs/rust_guidelines/performance.md), [M-TYPES-SEND](../../docs/rust_guidelines/libraries-interop.md)
- Related: [core/ownership.md](../core/ownership.md) (Arc, Mutex), [advanced/async.md](async.md) (async concurrency)
