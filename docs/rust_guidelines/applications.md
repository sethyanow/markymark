# Application Guidelines (progressive)

**TL;DR:** Use a single app-level error type (anyhow/eyre) and switch apps to mimalloc for easy perf wins.

**Checklist:**
- Pick one app-level error crate (anyhow/eyre) and use it consistently.
- Keep libraries on canonical error structs instead.
- Set `mimalloc` as the global allocator in applications.
- Avoid mixing multiple app-level error types.

## Applications may use Anyhow or Derivatives (M-APP-ERROR) { #M-APP-ERROR }

<why>To simplify application-level error handling.</why>
<version>0.1</version>

> Note, this guideline is primarily a relaxation and clarification of [M-ERRORS-CANONICAL-STRUCTS](./libraries-ux.md#M-ERRORS-CANONICAL-STRUCTS).

Applications, and crates in your own repository exclusively used from your application, may use [anyhow](https://github.com/dtolnay/anyhow), [eyre](https://github.com/eyre-rs/eyre) or similar application-level error crates instead of implementing their own types.

For example, in your application crates you may just re-export and use eyre's common `Result` type, which should be able to automatically handle all third party library errors, in particular the ones following [M-ERRORS-CANONICAL-STRUCTS](./libraries-ux.md#M-ERRORS-CANONICAL-STRUCTS).

```rust,ignore
use eyre::Result;

fn start_application() -> Result<()> {
    start_server()?;
    Ok(())
}
```

Once you selected your application error crate you should switch all application-level errors to that type, and you should not mix multiple application-level error types.

Libraries (crates used by more than one crate) should always follow [M-ERRORS-CANONICAL-STRUCTS](./libraries-ux.md#M-ERRORS-CANONICAL-STRUCTS) instead.

## Use Mimalloc for Apps (M-MIMALLOC-APPS) { #M-MIMALLOC-APPS }

<why>To get significant performance for free.</why>
<version>0.1</version>

Applications should set [mimalloc](https://crates.io/crates/mimalloc) as their global allocator. This usually results in notable performance increases along allocating hot paths; we have seen up to 25% benchmark improvements.

Add mimalloc to `Cargo.toml`:

```toml
[dependencies]
mimalloc = { version = "0.1" } # Or later version if available
```

Use it from `main.rs`:

```rust,ignore
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

### Related
- Library UX errors: `libraries-ux.md`
- Performance: `performance.md`
- Original: `../rust_guidelines_full.md`
