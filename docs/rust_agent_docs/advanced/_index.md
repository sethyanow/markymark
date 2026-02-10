## Advanced Topics — Overview

Advanced topics covering unsafe Rust, FFI, memory layout, concurrency, and async.
**Read the core/ files first** — these topics build on that foundation.

### Files

| File | Topic | When to Read |
|------|-------|-------------|
| [type-layout.md](type-layout.md) | repr(C/packed/transparent), alignment | FFI structs, memory optimization |
| [unsafe.md](unsafe.md) | 5 superpowers, safety invariants, PhantomData, Miri | Writing or reviewing unsafe code |
| [ffi.md](ffi.md) | extern "C", opaque pointers, DLL isolation | Interfacing with C/other languages |
| [concurrency.md](concurrency.md) | Threads, Send/Sync, atomics, channels, Rayon | Multi-threaded code |
| [async.md](async.md) | Future, async/await, pinning, executors | Asynchronous I/O |

### Reading Order

1. **type-layout.md** — Prerequisite for unsafe and FFI
2. **unsafe.md** — When and how to use unsafe
3. **ffi.md** — Foreign function interface (builds on layout + unsafe)
4. **concurrency.md** — Thread-based parallelism
5. **async.md** — Async/await model

### ⚠️ Critical Mistakes in This Section

| Mistake | Severity | File |
|---------|----------|------|
| References to packed struct fields | CRITICAL (UB) | [type-layout.md](type-layout.md) |
| String/Vec across FFI boundaries | CRITICAL (UB) | [ffi.md](ffi.md) |
| Wrong PhantomData variance | CRITICAL (unsound) | [unsafe.md](unsafe.md) |
| Wrong atomic ordering | HIGH | [concurrency.md](concurrency.md) |
| Ignoring pinning requirements | HIGH | [async.md](async.md) |
