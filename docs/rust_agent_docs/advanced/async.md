## Async Rust — Futures, Pinning & Executors

> **TL;DR:** `async fn` returns a `Future` that does nothing until `.await`ed. Use `tokio` as the
> default executor. Pinning is required for self-referential futures — use `Box::pin()` or
> `tokio::pin!()`. Always handle cancellation.

### Future Trait & async/await

```rust
// async fn returns impl Future<Output = T>
async fn fetch_data(url: &str) -> Result<String, reqwest::Error> {
    let body = reqwest::get(url).await?.text().await?;
    Ok(body)
}

// Explicit Future trait (rarely needed)
use std::future::Future;
fn make_future() -> impl Future<Output = i32> {
    async { 42 }
}
```

**Key insight:** Futures are lazy — calling `async fn` does NOT start execution.
The future only runs when polled by an executor (via `.await` or `spawn`).

### Executor Selection

| Executor | Use When |
|----------|----------|
| `tokio` | Default choice; full-featured, production-grade |
| `async-std` | When mirroring std API style is preferred |
| `smol` | Minimal, lightweight applications |
| Manual polling | Embedded systems, custom runtimes |

```rust
#[tokio::main]
async fn main() {
    let result = fetch_data("https://example.com").await;
    println!("{result:?}");
}
```

### Pinning Decision Tree

> ⚠️ **COMMON MISTAKE: Ignoring pinning requirements**
> Async functions create self-referential state machines. Moving such a future after it has
> been polled would invalidate internal pointers. `Pin` prevents this movement.

```
Does the compiler complain about pinning / Unpin?
├─ YES
│   Is it a future you OWN (created in your code)?
│   ├─ YES, on the heap → Box::pin(future)
│   ├─ YES, on the stack → tokio::pin!(future) or pin!()
│   └─ YES, stored in a struct → Pin<Box<dyn Future>>
│
│   Is it a stream or trait object?
│   └─ YES → Pin<Box<dyn Stream>> or Pin<Box<dyn Future>>
│
└─ NO (most async code — compiler handles it)
    └─ No action needed; .await handles pinning automatically
```

**Why async needs pinning:**
- `async {}` blocks compile into state machines that may hold references to their own local variables
- If the future is moved in memory after being polled, those internal references become dangling
- `Pin<P>` guarantees the pointee won't move, making self-references safe

```rust
use std::pin::Pin;
use std::future::Future;

// Storing a future in a struct
struct Task {
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
}

// Creating a pinned future
let task = Task {
    future: Box::pin(async {
        do_async_work().await;
    }),
};

// Stack pinning with tokio
async fn example() {
    let fut = async { 42 };
    tokio::pin!(fut);
    // fut is now pinned on the stack
    let val = (&mut fut).await;
}
```

### select! and join!

```rust
use tokio::{select, time::{sleep, Duration}};

// select! — race multiple futures, cancel losers
async fn fetch_with_timeout() -> Result<String, &'static str> {
    select! {
        result = fetch_data("https://api.example.com") => {
            result.map_err(|_| "fetch failed")
        }
        _ = sleep(Duration::from_secs(5)) => {
            Err("timeout")
        }
    }
}

// join! — run concurrently, wait for all
let (users, posts) = tokio::join!(
    fetch_users(),
    fetch_posts(),
);
```

### Cancellation and Timeouts

Dropping a future cancels it. This means:
- `.await` points are cancellation points
- Resources held across `.await` may not be cleaned up if cancelled
- Use `Drop` guards for critical cleanup

```rust
use tokio::time::timeout;

// Timeout wrapper
let result = timeout(
    Duration::from_secs(10),
    long_running_operation(),
).await;

match result {
    Ok(value) => println!("completed: {value:?}"),
    Err(_) => println!("timed out"),  // future was dropped/cancelled
}
```

### Sync ↔ Async Bridging

```rust
// Calling async from sync (using block_on)
fn sync_function() -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { fetch_data("url").await.unwrap() })
}

// Calling sync from async (using spawn_blocking)
async fn async_function() -> Vec<u8> {
    tokio::task::spawn_blocking(|| {
        // CPU-bound or blocking I/O work
        std::fs::read("large_file.bin").unwrap()
    }).await.unwrap()
}
```

**Rules:**
- Never call `block_on` inside an async context (deadlock)
- Use `spawn_blocking` for CPU-heavy work or blocking I/O in async context
- Add yield points (`tokio::task::yield_now().await`) in CPU-bound async loops

### Async Testing

```rust
// Tokio test — most common
#[tokio::test]
async fn test_fetch() {
    let result = fetch_data("https://httpbin.org/get").await;
    assert!(result.is_ok());
}

// With timeout (prevents hanging tests)
#[tokio::test]
async fn test_with_timeout() {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        fetch_data("https://httpbin.org/get"),
    ).await;
    assert!(result.is_ok(), "test timed out");
}

// Multi-threaded test runtime (default is current_thread)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_concurrent() {
    // Tests that require multiple threads
}
```

### Async Trait Patterns

`async fn` in traits requires the `async-trait` crate or Rust 1.75+ RPITIT:

```rust
// Rust 1.75+: native async fn in traits (not dyn-compatible)
trait DataStore {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&mut self, key: &str, value: String);
}

// For dyn-compatible async traits, use async-trait crate:
use async_trait::async_trait;

#[async_trait]
trait DynDataStore: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
}

// async_trait desugars to Pin<Box<dyn Future + Send + '_>>
```

### Structured Concurrency with JoinSet

Use `JoinSet` when spawning a dynamic number of tasks:

```rust
use tokio::task::JoinSet;

async fn process_all(urls: Vec<String>) -> Vec<String> {
    let mut set = JoinSet::new();

    for url in urls {
        set.spawn(async move {
            fetch_data(&url).await.unwrap_or_default()
        });
    }

    let mut results = Vec::new();
    while let Some(result) = set.join_next().await {
        match result {
            Ok(data) => results.push(data),
            Err(e) => eprintln!("task panicked: {e}"),
        }
    }
    results
}
```

### Cancellation Safety

> ⚠️ **COMMON MISTAKE: Ignoring cancellation safety in `select!`**
> When `select!` completes one branch, all other futures are **dropped**.
> If a future has done partial work (e.g., read half a message), that work is lost.

```
Is your future cancellation-safe?
├─ It only does a single .await at the end?
│   └─ YES → safe (no partial state)
├─ It modifies external state between .await points?
│   └─ UNSAFE — state may be inconsistent on cancel
│       Fix: Use select!-compatible APIs (e.g., tokio::sync::mpsc::Receiver::recv)
├─ It holds a lock across .await?
│   └─ UNSAFE — lock won't be released on cancel
│       Fix: Scope locks before .await
└─ Unsure?
    └─ Don't use it in select! — wrap in spawn() instead
```

### References

- The Rust Book: [Async](https://doc.rust-lang.org/book/ch17-00-async-await.html)
- Tokio Tutorial: [tokio.rs](https://tokio.rs/tokio/tutorial)
- Guidelines: [M-YIELD-POINTS](../../docs/rust_guidelines/performance.md)
- Related: [advanced/concurrency.md](concurrency.md) (threads, Send/Sync), [core/ownership.md](../core/ownership.md) (lifetimes in async), [core/closures.md](../core/closures.md) (move closures for spawn)
