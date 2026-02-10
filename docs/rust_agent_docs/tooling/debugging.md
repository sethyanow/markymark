## Debugging — Compiler Errors, Tracing & Miri

> **TL;DR:** Read compiler error messages carefully — they usually tell you the fix.
> Use `dbg!()` for quick inspection, `tracing` for production logging, and Miri for
> detecting undefined behavior in unsafe code.

### Reading Compiler Errors

Rust's compiler errors are highly informative. Follow this process:

1. **Read the error message** — it often contains the fix
2. **Look at the "help:" line** — Rust suggests corrections
3. **Check the error code** — `rustc --explain E0308` for detailed explanation
4. **Read the span** — arrows point to the exact problem location

```
error[E0382]: borrow of moved value: `s`
 --> src/main.rs:4:20
  |
2 |     let s = String::from("hello");
  |         - move occurs because `s` has type `String`
3 |     let s2 = s;
  |              - value moved here
4 |     println!("{}", s);
  |                    ^ value borrowed here after move
  |
help: consider cloning the value
  |
3 |     let s2 = s.clone();
  |               ++++++++
```

### Common Compiler Error Solutions

See [reference/compiler-errors.md](../reference/compiler-errors.md) for a comprehensive table.

### dbg! Macro

```rust
let value = dbg!(calculate_something()); // Prints file:line = value to stderr
// Output: [src/main.rs:5] calculate_something() = 42

// Chain in expressions
let result = dbg!(dbg!(a) + dbg!(b));
```

`dbg!()` returns the value, so it can be inserted anywhere without changing logic.
**Remove before committing** — it prints to stderr unconditionally.

### tracing for Production Logging

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(password))]  // Log function entry/exit, skip sensitive fields
fn authenticate(username: &str, password: &str) -> Result<Token, AuthError> {
    info!(username, "authentication attempt");

    let result = verify_credentials(username, password);

    match &result {
        Ok(_) => info!(username, "authentication successful"),
        Err(e) => warn!(username, error = %e, "authentication failed"),
    }

    result
}
```

```rust
// Setup in main
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()  // Structured JSON output
        .init();
}
```

Set log level: `RUST_LOG=debug cargo run`

### Miri for Unsafe Code

```bash
rustup +nightly component add miri
cargo +nightly miri test
```

Miri detects:
- Out-of-bounds memory access
- Use-after-free
- Invalid alignment / unaligned references
- Data races (`-Zmiri-data-race`)
- Memory leaks (`-Zmiri-leak-check`)
- Violations of stacked borrows (`-Zmiri-tag-raw-pointers`)

### RUST_BACKTRACE

```bash
RUST_BACKTRACE=1 cargo run     # Short backtrace
RUST_BACKTRACE=full cargo run  # Full backtrace with all frames
```

### cargo-expand

Expand macros to see generated code:

```bash
cargo install cargo-expand
cargo expand                    # Expand all macros
cargo expand my_module          # Expand specific module
```

### References

- Compiler errors: [rust-lang.org/tools/install](https://doc.rust-lang.org/error_codes/error-index.html)
- Miri: [github.com/rust-lang/miri](https://github.com/rust-lang/miri)
- tracing: [docs.rs/tracing](https://docs.rs/tracing/)
- Related: [reference/compiler-errors.md](../reference/compiler-errors.md), [advanced/unsafe.md](../advanced/unsafe.md) (Miri)
