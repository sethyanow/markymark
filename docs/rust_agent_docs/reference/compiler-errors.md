## Compiler Error Quick Reference

> **TL;DR:** Common compiler errors mapped to causes and fixes. Use `rustc --explain EXXXX`
> for full explanations.

### Ownership & Borrowing Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0382 | Use of moved value | Value ownership transferred | Clone, borrow, or restructure |
| E0505 | Cannot move out while borrowed | Move while borrow is active | Reduce borrow scope |
| E0502 | Cannot borrow as mutable | Shared borrow exists | Restructure; use interior mutability |
| E0499 | Cannot borrow as mutable more than once | Two `&mut` to same data | Split borrows or reduce scope |
| E0507 | Cannot move out of borrowed content | Move from behind reference | Clone, `std::mem::take`, or `std::mem::replace` |
| E0515 | Cannot return reference to temporary | Reference outlives temporary | Return owned type |
| E0597 | Does not live long enough | Reference outlives referent | Extend life of data, use `'static`, or clone |
| E0106 | Missing lifetime specifier | Compiler can't infer lifetimes | Add explicit lifetime annotations |

### Type & Trait Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0308 | Mismatched types | Type mismatch | Check types, add conversion (.into(), as) |
| E0277 | Trait not satisfied | Missing trait bound | Add bound, implement trait, or derive |
| E0412 | Cannot find type | Unknown type name | Import type, check spelling |
| E0599 | Method not found | No such method on type | Check type, import trait, check deref |
| E0609 | No field on type | Accessing nonexistent field | Check struct definition |
| E0425 | Cannot find value | Unresolved name | Import, check scope, check spelling |
| E0433 | Failed to resolve | Module/type path wrong | Check use statement |
| E0061 | Wrong number of args | Argument count mismatch | Check function signature |
| E0369 | Binary operation not supported | Operator not implemented | Implement `Add`, `PartialEq`, etc. |

### Lifetime & Reference Errors

| Code | Message | Cause | Fix |
|------|---------|-------|-----|
| E0621 | Explicit lifetime required in type | Parameters need lifetimes | Annotate lifetime on parameter |
| E0623 | Lifetime mismatch | Return type lifetime differs from args | Align lifetime annotations |
| E0716 | Temporary value dropped while borrowed | Borrow of temporary | Bind temporary to a variable first |

### Send/Sync Errors

| Message Pattern | Cause | Fix |
|----------------|-------|-----|
| "cannot be sent between threads safely" | Type is `!Send` | Use `Arc` instead of `Rc`; remove `RefCell` |
| "not safe to share between threads" | Type is `!Sync` | Use `Mutex`/`RwLock` for synchronization |
| "`dyn Future` is not `Send`" | Future holds `!Send` type across await | Scope `!Send` variables; drop before `.await` |

### Quick Diagnostic Steps

1. Read the **full error message** — Rust's errors are excellent
2. Check the **"help:" suggestion** — often gives the exact fix
3. Look at the **span** — arrows point to the problem
4. Run `rustc --explain EXXXX` — detailed explanation with examples
5. Check this table for common patterns

---

### Error Walkthroughs

Step-by-step examples of reading and resolving real compiler errors.
Each walkthrough shows the actual error output, how to read it, and how to fix it.

#### Walkthrough 1: E0382 — Use of moved value

**The code:**
```rust
fn process(data: Vec<String>) {
    let backup = data;
    for item in data {          // ERROR here
        println!("{item}");
    }
}
```

**The error:**
```text
error[E0382]: use of moved value: `data`
 --> src/main.rs:4:17
  |
2 |     let backup = data;
  |                  ---- value moved here
3 |     for item in data {
  |                 ^^^^ value used here after move
  |
  = note: move occurs because `data` has type `Vec<String>`,
          which does not implement the `Copy` trait
```

**How to read it:**

1. **Error code `E0382`** — "use of moved value." Ownership was transferred.
2. **First span** (line 2) — `---- value moved here`. The assignment `let backup = data` transferred ownership of `data` to `backup`.
3. **Second span** (line 3) — `^^^^ value used here after move`. After the move, `data` is no longer valid.
4. **Note** — Tells you *why* it moved: `Vec<String>` doesn't implement `Copy`, so assignment moves instead of copies.

**Fixes (choose one):**
```rust
// Option A: Clone if you need both copies
let backup = data.clone();

// Option B: Borrow instead of moving
let backup = &data;

// Option C: Use backup instead of data after the move
let backup = data;
for item in &backup { /* ... */ }
```

---

#### Walkthrough 2: E0502 — Cannot borrow as mutable because also borrowed as immutable

**The code:**
```rust
fn update_map(map: &mut HashMap<String, Vec<String>>) {
    let defaults = map.get("defaults").unwrap();  // immutable borrow
    map.insert("key".into(), defaults.clone());   // ERROR: mutable borrow
}
```

**The error:**
```text
error[E0502]: cannot borrow `*map` as mutable because it is also
              borrowed as immutable
 --> src/main.rs:4:5
  |
3 |     let defaults = map.get("defaults").unwrap();
  |                    --- immutable borrow occurs here
4 |     map.insert("key".into(), defaults.clone());
  |     ^^^ ------- immutable borrow later used here
  |     |
  |     mutable borrow occurs here
```

**How to read it:**

1. **Line 3** — `map.get()` borrows `map` immutably. The returned reference `defaults` keeps this borrow alive.
2. **Line 4** — `map.insert()` needs `&mut map`. Can't have `&mut` while `&` exists.
3. **"immutable borrow later used here"** — `defaults.clone()` on the same line uses the immutable borrow, so it's still active during `insert`.

**Fix — break the borrow dependency:**
```rust
fn update_map(map: &mut HashMap<String, Vec<String>>) {
    // Clone the data BEFORE mutating the map
    let defaults = map.get("defaults").unwrap().clone();  // borrow ends here
    map.insert("key".into(), defaults);  // now safe to mutate
}
```

**Key insight:** The immutable borrow's scope extends to its *last use*, not to the end of the block (NLL — Non-Lexical Lifetimes). Clone or copy the data so the borrow ends before you mutate.

---

#### Walkthrough 3: E0277 — Trait bound not satisfied (`Send` in async context)

**The code:**
```rust
use std::rc::Rc;

async fn process(data: Rc<Vec<String>>) {
    tokio::spawn(async move {   // ERROR here
        println!("{}", data.len());
    });
}
```

**The error:**
```text
error[E0277]: `Rc<Vec<String>>` cannot be sent between threads safely
   --> src/main.rs:4:18
    |
4   |     tokio::spawn(async move {
    |     ------------ ^^^^^^^^^^^^
    |     |
    |     required by a bound introduced by this call
    |
    = help: within `{async block}`, the trait `Send` is not
            implemented for `Rc<Vec<String>>`
    = note: required because it appears within the type
            `{async block}`
note: required by a bound in `tokio::spawn`
   --> tokio/src/task/spawn.rs
    |
    |     T: Future + Send + 'static,
    |                 ^^^^ required by this bound in `spawn`
```

**How to read it:**

1. **"cannot be sent between threads safely"** — The spawned future must be `Send` because `tokio::spawn` can run it on any thread.
2. **"the trait `Send` is not implemented for `Rc<Vec<String>>`"** — `Rc` uses a non-atomic reference count, so it's `!Send`.
3. **"required by a bound in `tokio::spawn`"** — The note shows the exact bound: `T: Future + Send + 'static`.
4. **Trace the chain:** `Rc` is `!Send` → the async block captures `Rc` → the async block is `!Send` → `tokio::spawn` requires `Send` → error.

**Fix:**
```rust
use std::sync::Arc;

async fn process(data: Arc<Vec<String>>) {
    tokio::spawn(async move {
        println!("{}", data.len());
    });
}
```

**Key insight:** When you see "cannot be sent between threads safely," trace the `!Send` type from the error message. Replace `Rc` → `Arc`, `RefCell` → `Mutex`, or scope `!Send` values so they don't live across `.await`.

---

#### Walkthrough 4: E0716 — Temporary value dropped while borrowed

> **Note:** This signature also triggers E0106 (missing lifetime specifier) since
> there are no input references to elide from. The walkthrough assumes you've added
> a lifetime annotation (e.g. `fn get_name<'a>(...) -> &'a str`) and hit E0716 next.

**The code:**
```rust
fn get_name(use_default: bool) -> &str {
    if use_default {
        &String::from("default")   // ERROR: temporary
    } else {
        "custom"
    }
}
```

**The error:**
```text
error[E0716]: temporary value dropped while borrowed
 --> src/main.rs:3:10
  |
3 |         &String::from("default")
  |          ^^^^^^^^^^^^^^^^^^^^^^^ - temporary value is freed
  |          |                         at the end of this statement
  |          creates a temporary which is freed while still in use
  |
  = note: consider using a `let` binding to create a longer lived value
```

**How to read it:**

1. **"temporary value"** — `String::from("default")` creates a temporary `String` on the stack.
2. **"freed at the end of this statement"** — The temporary is dropped immediately after the expression.
3. **"creates a temporary which is freed while still in use"** — We're trying to return a reference to it, but it won't exist after the function returns.
4. **The help** — "consider using a `let` binding" — binding to a variable extends the temporary's lifetime.

**But binding alone isn't enough here** — the function returns `&str`, so the data must outlive the function. Options:

```rust
// Option A: Return a string literal (lives forever)
fn get_name(use_default: bool) -> &'static str {
    if use_default { "default" } else { "custom" }
}

// Option B: Return an owned String
fn get_name(use_default: bool) -> String {
    if use_default { "default".into() } else { "custom".into() }
}

// Option C: Use Cow for zero-cost when possible
fn get_name(use_default: bool) -> Cow<'static, str> {
    if use_default { Cow::Borrowed("default") } else { Cow::Borrowed("custom") }
}
```

**Key insight:** When a function creates data and tries to return a reference to it, you need to either return an owned type (`String`), use a `'static` reference (string literals), or accept a lifetime parameter that ties the reference to caller-provided data.

### Reading Any Error — The Protocol

```text
1. ERROR LINE → What code triggered it?
2. SPANS (---- and ^^^^) → Where are the conflicting operations?
3. NOTES (= note:) → Why does this constraint exist?
4. HELP (= help:) → What does the compiler suggest?
5. REQUIRED BY → Which function/trait imposed the bound?
```

When errors cascade (multiple errors from one root cause), fix the **first** error and recompile. Later errors often disappear.

### References

- Error Index: [doc.rust-lang.org/error_codes](https://doc.rust-lang.org/error_codes/error-index.html)
- Related: [core/ownership.md](../core/ownership.md), [core/traits.md](../core/traits.md), [tooling/debugging.md](../tooling/debugging.md)
