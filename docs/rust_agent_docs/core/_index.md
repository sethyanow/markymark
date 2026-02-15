## Core Language — Overview

The core language files cover fundamental Rust concepts every agent must understand
before writing any Rust code.

### Files

| File | Topic | When to Read |
|------|-------|-------------|
| [ownership.md](ownership.md) | Ownership, borrowing, lifetimes, smart pointers | Always — Rust's core differentiator |
| [types.md](types.md) | Primitives, structs, enums, generics, pattern matching | Defining data structures |
| [traits.md](traits.md) | Trait system, std traits, object safety, orphan rules | Polymorphism, implementing std traits |
| [errors.md](errors.md) | Option, Result, `?`, thiserror, anyhow | Any error handling |
| [closures.md](closures.md) | Fn/FnMut/FnOnce, capture semantics, move | Callbacks, iterators, threads |
| [collections.md](collections.md) | Vec, HashMap, iterators, string types | Choosing containers or string types |
| [modules.md](modules.md) | mod system, visibility, workspaces, features | Organizing code |

### Reading Order

1. **ownership.md** — Must-read; everything in Rust flows from ownership
2. **types.md** — How to define and compose data
3. **traits.md** — How to abstract behavior
4. **closures.md** — How Fn traits interact with ownership
5. **errors.md** — How to handle failure
6. **collections.md** — How to work with data
7. **modules.md** — How to organize code at scale

### Common Tasks → File

| Task | File |
|------|------|
| Fix borrow checker error | [ownership.md](ownership.md) |
| Choose between `&T`, `Box<T>`, `Rc<T>`, `Arc<T>` | [ownership.md](ownership.md) |
| Write a function that takes a callback | [closures.md](closures.md) |
| Implement `Display`, `From`, `Iterator` | [traits.md](traits.md) |
| Decide `thiserror` vs `anyhow` | [errors.md](errors.md) |
| Pick `String` vs `&str` vs `OsString` | [collections.md](collections.md) |
| Set up feature flags | [modules.md](modules.md) |
