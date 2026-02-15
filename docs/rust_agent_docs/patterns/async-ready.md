## Make Your Type Async-Ready

> **TL;DR:** A checklist for making types work in async Rust. Combines Send, Sync, Pin,
> lifetimes, and interior mutability into a single decision framework.

### The Async-Ready Checklist

Before using a type in async code (across `.await` points, in `tokio::spawn`, in shared state),
verify each property:

| Property | Required When | How to Check |
|----------|--------------|--------------|
| `Send` | Type crosses thread boundaries (`tokio::spawn`, `thread::spawn`) | All fields must be `Send` |
| `Sync` | Shared reference `&T` sent between threads (`Arc<T>`) | All fields must be `Sync` |
| `'static` | Type stored in spawned task or `'static` context | No borrowed references (all owned data) |
| `Unpin` | Type used as `dyn Future` or stored in `Pin<Box<_>>` | Most types are `Unpin` by default |

### Decision Tree: Is My Type Async-Ready?

```
Will this type be used in async code?
├─ Stored in struct held across .await?
│   └─ Must be Send (if task is spawned) + owned (no borrows)
├─ Passed to tokio::spawn()?
│   └─ Must be Send + 'static
├─ Shared via Arc<T> across tasks?
│   └─ T must be Send + Sync
├─ Used as trait object (dyn Future / dyn Stream)?
│   └─ Must be Unpin (or wrap in Pin<Box<_>>)
└─ Only used within a single async fn, not held across .await?
    └─ No special requirements
```

### Send: Can It Cross Thread Boundaries?

A type is `Send` if it can be safely transferred to another thread. Auto-derived when all fields are `Send`.

**Common `!Send` types and fixes:**

| `!Send` Type | Why | Fix |
|-------------|-----|-----|
| `Rc<T>` | Non-atomic refcount | Use `Arc<T>` |
| `*mut T`, `*const T` | Raw pointers | Wrap in newtype, `unsafe impl Send` if safe |
| `&T` where `T: !Sync` | Shared ref to `!Sync` data | Use owned `T` or `Arc<T>` |
| `MutexGuard<T>` (std) | Some OS mutexes are `!Send` | Drop guard before `.await` |
| `&bumpalo::Bump` | `Bump: !Sync` so `&Bump: !Send` | Keep arena refs in single-task scope |
| `RefCell<T>` | `Send` but `!Sync` — error may actually be about `Sync` | Use `Mutex<T>` for thread-sharing |

**Tracing a `!Send` chain (real example):**
```
// Arena-backed HashMap from the markymark parser:
ArenaHashMap<K, V>
  └─ hashbrown::HashMap<K, V, DefaultHashBuilder, &Bump>
       └─ &Bump  (the allocator reference)
            └─ Bump: !Sync
                 └─ &T where T: !Sync → !Send
// Result: ArenaHashMap is !Send because &Bump is !Send because Bump is !Sync
```

**Fix strategy:** Keep `!Send` types in the parser layer. Convert to owned types before
crossing into async/threaded code (index layer uses `std::collections::HashMap`).

### Sync: Can Shared References Cross Threads?

A type is `Sync` if `&T` can be safely shared between threads. Auto-derived when all fields are `Sync`.

`T: Sync` means `&T: Send`. These are linked:

```text
Send + Sync:  Arc<T>, Mutex<T>, AtomicU64, String, Vec<T>
Send + !Sync: Cell<T>, RefCell<T>, mpsc::Sender, bumpalo::Bump
!Send + Sync: (rare — usually indicates a design issue)
!Send + !Sync: Rc<T>, *mut T
```

**Common pattern — making shared state Sync:**
```rust
// ❌ NOT Sync: interior mutability without synchronization
struct State { data: RefCell<Vec<String>> }

// ✅ Sync: mutex-protected interior mutability
struct State { data: Mutex<Vec<String>> }

// ✅ Also Sync: read-write lock for read-heavy access
struct State { data: RwLock<Vec<String>> }
```

### 'static: No Borrowed References

`tokio::spawn` requires `Future + Send + 'static`. The `'static` bound means the future
must not borrow data from the caller's stack.

```rust
// ❌ Borrows `data` from caller's stack — not 'static
async fn process(data: &[String]) {
    tokio::spawn(async {
        println!("{}", data.len());  // ERROR: data is &[String], not 'static
    });
}

// ✅ Move owned data into the spawned task
async fn process(data: Vec<String>) {
    tokio::spawn(async move {
        println!("{}", data.len());  // OK: data is owned, moved into task
    });
}

// ✅ Or clone+move if you need data in both places
async fn process(data: &[String]) {
    let owned = data.to_vec();
    tokio::spawn(async move {
        println!("{}", owned.len());
    });
}
```

### Pin: When Types Must Not Move

Most types are `Unpin` (can be moved freely). Pinning only matters when:

1. You create a self-referential future (rare in application code)
2. You store `dyn Future` as a trait object
3. You use `select!` or `pin!` macros

```rust
// When you need Pin — trait objects
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

// In a struct that holds futures
struct TaskQueue {
    pending: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

// Most async code: no Pin needed
// .await handles pinning automatically
async fn simple() {
    let result = some_async_fn().await;  // no Pin needed here
}
```

### Combining Properties: The Full Picture

**Scenario: Shared state in a web server**
```rust
// All four properties needed:
// - Send: handler runs on different thread than main
// - Sync: multiple handlers share &AppState via Arc
// - 'static: state outlives any individual request
// - No Pin needed (not a Future itself)

struct AppState {
    db: Pool,           // must be Send + Sync
    cache: Mutex<HashMap<String, String>>,  // Mutex makes it Sync
    config: Config,     // must be Send + Sync (all owned fields)
}

// Arc<AppState> satisfies: Send + Sync + 'static
let state = Arc::new(AppState { /* ... */ });
```

**Scenario: Background task processing**
```rust
// Task must be Send + 'static for tokio::spawn
// Interior data must be owned (no borrows)

struct Task {
    id: u64,
    payload: String,        // owned — OK
    // data: &'a str,       // ❌ borrowed — not 'static
    // cache: Rc<Cache>,    // ❌ Rc — not Send
    cache: Arc<Cache>,      // ✅ Arc — Send + Sync
}

async fn run_task(task: Task) {
    tokio::spawn(async move {
        process(task).await;  // task is Send + 'static ✓
    });
}
```

**Scenario: Parser with arena allocation (from markymark)**
```rust
// Arena types are !Send — kept in parser layer only.
// Index layer converts to owned types for async/LSP use.

// Parser layer (single-threaded, !Send OK):
struct Ast<'arena> {
    arena: DocumentArena,                    // borrows from arena make struct !Send
    headings: &'arena [Heading<'arena>],     // borrows from arena
}

// Index layer (must be Send for LSP):
struct DocumentIndex {
    arena: DocumentArena,                    // owns the arena
    headings: Vec<HeadingEntry>,             // owned data, no arena refs
    tags: HashMap<String, XmlTagEntry>,      // std HashMap, not ArenaHashMap
}
// DocumentIndex: Send ✓ (all fields are Send)
```

### Quick Reference: Common Async Patterns

| Pattern | Send | Sync | 'static | Pinning |
|---------|------|------|---------|---------|
| `tokio::spawn(future)` | future: Send | -- | future: 'static | auto |
| `Arc<T>` shared state | T: Send | T: Sync | T: 'static | -- |
| `Mutex<T>` interior mut | T: Send | yes | -- | -- |
| `Box<dyn Future>` | if `+ Send` | -- | if `+ 'static` | `Pin<Box<_>>` |
| `select!` branches | each: Send | -- | -- | `pin!()` local futures |
| Channel `mpsc::Sender` | Send | !Sync | -- | -- |

### References

- Related: [../advanced/async.md](../advanced/async.md) (async fundamentals)
- Related: [../advanced/concurrency.md](../advanced/concurrency.md) (Send/Sync deep dive)
- Related: [../core/ownership.md](../core/ownership.md) (lifetimes and borrowing)
- Decision tree: [../reference/decision-trees.md](../reference/decision-trees.md) (Why Is My Type !Send)
