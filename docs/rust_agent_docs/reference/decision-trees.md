## Decision Trees — Collected Reference

> **TL;DR:** All decision trees from the documentation collected in one place.
> Each links back to its source file for full context.

### Which Smart Pointer?
*Source: [core/ownership.md](../core/ownership.md)*

```
Do you need shared ownership?
├─ NO
│   Need heap allocation?
│   ├─ YES → Box<T>
│   └─ NO → just use T (stack)
└─ YES
    Is it single-threaded?
    ├─ YES → Rc<T>
    │   Need interior mutability?
    │   ├─ YES → Rc<RefCell<T>>
    │   └─ NO → Rc<T>
    └─ NO (multi-threaded) → Arc<T>
        Need interior mutability?
        ├─ YES, simple writes → Arc<Mutex<T>>
        ├─ YES, read-heavy → Arc<RwLock<T>>
        └─ NO → Arc<T>

Need interior mutability WITHOUT shared ownership?
├─ Single-threaded, Copy types → Cell<T>
├─ Single-threaded, non-Copy → RefCell<T>
├─ Multi-threaded → Mutex<T> or RwLock<T>
└─ Lock-free atomic → AtomicT (primitives only)
```

### Associated Types vs Generic Parameters?
*Source: [core/types.md](../core/types.md)*

```
Does the implementor choose the type?
├─ YES (one impl per concrete type choice)
│   └─ Use associated type: type Output;
│       Example: Iterator::Item, Add::Output
└─ NO (generic over many types simultaneously)
    └─ Use generic parameter: Trait<T>
        Example: From<T>, AsRef<T>
```

### Which Conversion Trait?
*Source: [core/traits.md](../core/traits.md)*

```
Converting between types?
├─ Cheap reference conversion (no allocation)?
│   ├─ Immutable → AsRef<T>
│   └─ Mutable → AsMut<T>
├─ Owned type conversion?
│   ├─ Infallible → From<T> / Into<T>
│   │   └─ Implement From<T> (Into is auto-derived)
│   └─ Fallible → TryFrom<T> / TryInto<T>
├─ Borrowing with possible ownership? → Borrow<T> / ToOwned
└─ Smart pointer-like coercion? → Deref / DerefMut
```

### Library vs Application Errors?
*Source: [core/errors.md](../core/errors.md)*

```
Are you writing a library (used by other crates)?
├─ YES → Use thiserror with descriptive error structs
│   ├─ One error enum per module or logical group
│   ├─ Implement std::error::Error, Display, Debug
│   ├─ ❌ Do NOT use anyhow/eyre in library code
│   └─ ❌ Do NOT use unwrap()/expect()
└─ NO (application code)
    └─ Use anyhow or eyre
        ├─ Re-export Result type: use anyhow::Result;
        ├─ Add context: .context("failed to read config")?
        └─ Library errors auto-convert via From
```

### Which String Type?
*Source: [core/collections.md](../core/collections.md)*

```
What kind of string do you need?
├─ Owned, heap-allocated, growable?
│   └─ String
├─ Borrowed slice of UTF-8 text?
│   └─ &str
├─ OS-native string (may not be UTF-8)?
│   ├─ Owned → OsString
│   └─ Borrowed → &OsStr
├─ File system path?
│   ├─ Owned → PathBuf
│   └─ Borrowed → &Path
├─ C-compatible null-terminated string?
│   ├─ Owned (Rust → C) → CString
│   └─ Borrowed (C → Rust) → &CStr
└─ Raw bytes (not necessarily text)?
    ├─ Owned → Vec<u8>
    └─ Borrowed → &[u8]
```

### Which Atomic Ordering?
*Source: [advanced/concurrency.md](../advanced/concurrency.md)*

```
What are you doing with the atomic?
├─ Simple counter (no synchronization with other data)?
│   └─ Relaxed is OK
├─ Publishing data (writer side)?
│   └─ Release
├─ Consuming data (reader side)?
│   └─ Acquire
├─ Read-modify-write (compare_exchange, fetch_add)?
│   └─ AcqRel
├─ Need total ordering across ALL threads?
│   └─ SeqCst
└─ Unsure?
    └─ SeqCst — correctness first, optimize later
```

### When Is Pinning Needed?
*Source: [advanced/async.md](../advanced/async.md)*

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

### Which Pattern To Use?
*Source: [patterns/idioms.md](../patterns/idioms.md)*

```
What problem are you solving?
├─ Complex object construction (>3 params)?
│   └─ Builder pattern
├─ Adding semantic meaning to a primitive?
│   └─ Newtype pattern
├─ Enforcing valid state transitions at compile time?
│   └─ Typestate pattern
├─ Resource cleanup on scope exit?
│   └─ RAII / Drop pattern
├─ Avoiding allocation when data might be borrowed OR owned?
│   └─ Cow (Clone-on-Write)
├─ Smart pointer-like access to an inner type?
│   └─ Deref polymorphism
├─ Adding methods to a foreign type?
│   └─ Extension trait
├─ Preventing external implementations of a trait?
│   └─ Sealed trait
└─ Unsure?
    └─ Start with the simplest approach; refactor as needed
```

### macro_rules! vs Proc Macro vs Generics?
*Source: [tooling/macros.md](../tooling/macros.md)*

```
Can you solve it with generics + traits?
├─ YES → Use generics (simpler, better errors, IDE support)
└─ NO
    Do you need to generate code from struct/enum shape?
    ├─ YES → Use derive proc macro (#[derive(MyTrait)])
    └─ NO
        Do you need compile-time code repetition/patterns?
        ├─ YES → Use macro_rules!
        └─ NO
            Do you need to transform arbitrary syntax?
            └─ YES → Use attribute proc macro (#[my_attr])
```

### Which Fn Trait Bound?
*Source: [core/closures.md](../core/closures.md)*

```
How will you call the closure?
├─ Exactly once (consuming callback, one-shot handler)?
│   └─ FnOnce — most flexible, accepts all closures
├─ Multiple times, closure may need to mutate state?
│   └─ FnMut — accepts Fn and FnMut closures
├─ Multiple times, possibly concurrently, no mutation?
│   └─ Fn — most restrictive, guarantees no side effects
└─ Unsure?
    └─ Start with FnOnce (accepts everything); tighten if needed
```

### Why Is My Type !Send?
*Source: [advanced/concurrency.md](../advanced/concurrency.md)*

```
My struct is !Send — why?
├─ Contains Rc<T>? → Rc is !Send (non-atomic refcount)
│   └─ Fix: Use Arc<T> instead
├─ Contains *mut T or *const T? → Raw pointers are !Send
│   └─ Fix: Wrap in newtype, unsafe impl Send if safe
├─ Contains Cell<T> or RefCell<T>? → These are Send but !Sync
│   └─ Check: is the error actually about Sync, not Send?
├─ Contains &T where T: !Sync? → &T is !Send when T: !Sync
│   └─ Fix: Use owned T or Arc<T> instead of &T
└─ Contains a type with a !Send field (transitive)?
    └─ Trace deeper: which field of that type is !Send?
```

### Is My Future Cancellation-Safe?
*Source: [advanced/async.md](../advanced/async.md)*

```
Is your future cancellation-safe?
├─ It only does a single .await at the end?
│   └─ YES → safe (no partial state)
├─ It modifies external state between .await points?
│   └─ UNSAFE — state may be inconsistent on cancel
│       Fix: Use select!-compatible APIs (e.g., mpsc::Receiver::recv)
├─ It holds a lock across .await?
│   └─ UNSAFE — lock won't be released on cancel
│       Fix: Scope locks before .await
└─ Unsure?
    └─ Don't use it in select! — wrap in spawn() instead
```

### References

- Each decision tree links to its source file above for full context
- Related: All source files listed per tree
