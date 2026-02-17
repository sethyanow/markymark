# bumpalo Pitfalls

Common gotchas and mistakes when using bumpalo arena allocation.

[← Back to bumpalo.md](../bumpalo.md)

---

## Lifetimes Must Match Arena

<pitfall>
**Problem:** Data allocated in arena cannot outlive the arena.

```rust
// BAD: Returns reference to arena-local data
fn parse_heading(content: &str) -> &str {
    let arena = Bump::new();
    let heading = arena.alloc_str(&content[0..10]);
    heading // ERROR: arena dropped, heading invalid!
}
```

**Solution:** Pass arena from caller or return owned data:

```rust
// GOOD: Arena lifetime flows through
fn parse_heading<'a>(arena: &'a Bump, content: &str) -> &'a str {
    arena.alloc_str(&content[0..10])
}

// Or return owned
fn parse_heading_owned(content: &str) -> String {
    content[0..10].to_string()
}
```
</pitfall>

## No Individual Deallocation

<pitfall>
**Problem:** Cannot free individual allocations.

```rust
// BAD: Expecting to free individual items
let arena = Bump::new();
let a = arena.alloc(1);
let b = arena.alloc(2);
// Cannot free just 'a' - must drop entire arena

// This is a design feature, not a bug!
```

**Solution:** Design around bulk deallocation:

```rust
// GOOD: One arena per logical unit that's freed together
struct Request {
    arena: Bump,
    // ... request-scoped data
}

// When request completes, drop Request, arena freed
```
</pitfall>

## Drop Not Called

<pitfall>
**Problem:** `Bump::alloc` doesn't call `Drop` on allocated values.

```rust
struct NeedsDrop {
    data: String, // Has Drop impl
}

let arena = Bump::new();
let x = arena.alloc(NeedsDrop { data: "hello".into() });
// When arena dropped, NeedsDrop::drop() is NOT called!
// The String inside is leaked!
```

**Solution:** Only allocate types that don't need Drop, or use `alloc_with`:

```rust
// GOOD: Use types without Drop
struct NoDrop<'a> {
    data: &'a str, // Borrowed, no Drop
}

// Or use bumpalo's collections which handle this
use bumpalo::collections::String as BumpString;
let s = BumpString::from_str_in("hello", &arena);
// BumpString doesn't heap-allocate, so no leak
```

For types that need Drop, use `bumpalo::boxed::Box`:

```rust
use bumpalo::boxed::Box as BumpBox;

let arena = Bump::new();
let boxed = BumpBox::new_in(NeedsDrop { data: "hello".into() }, &arena);
// Drop WILL be called when arena is dropped (with "boxed" feature)
```
</pitfall>

## Thread Safety

<pitfall>
**Problem:** `Bump` is not `Sync` - cannot be shared across threads.

```rust
// BAD: Sharing arena across threads
let arena = Arc::new(Bump::new());
let arena_clone = arena.clone();
std::thread::spawn(move || {
    arena_clone.alloc(1); // ERROR: Bump is not Sync
});
```

**Solution:** Use one arena per thread or protect with mutex (defeats purpose):

```rust
// GOOD: Thread-local arenas
thread_local! {
    static ARENA: RefCell<Bump> = RefCell::new(Bump::new());
}

fn alloc_in_thread<T>(value: T) -> *mut T {
    ARENA.with(|arena| {
        arena.borrow().alloc(value) as *mut T
    })
}
```
</pitfall>

## Arena Capacity Growth

<pitfall>
**Problem:** Arena grows but never shrinks until dropped.

```rust
let arena = Bump::new();
// Allocate 1GB
let _big = arena.alloc_slice_fill_copy(1_000_000_000, 0u8);
arena.reset(); // Frees logically, but capacity stays at 1GB!
```

**Solution:** Drop and recreate arena to release memory:

```rust
// If you need to release memory:
fn process_with_fresh_arena() {
    let arena = Bump::new();
    // ... use arena ...
} // Arena dropped, memory released

// Or use with_capacity for predictable size:
let arena = Bump::with_capacity(1024 * 1024); // 1MB initial
```
</pitfall>

---

## Related

- [bumpalo.md](../bumpalo.md) - Main documentation
- [advanced.md](advanced.md) - Real-world patterns
- bumpalo docs: https://docs.rs/bumpalo/
