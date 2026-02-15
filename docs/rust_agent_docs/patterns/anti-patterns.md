## Anti-Patterns — What NOT to Do

> **TL;DR:** Avoid stringly-typed APIs, excessive `unwrap()`, cloning as a crutch, fighting
> the borrow checker, unnecessary heap allocations, ignoring clippy, glob imports,
> leaky abstractions, and excessive generics. Each entry shows the fix.

### Anti-Pattern to Idiomatic Fix Table

| Anti-Pattern | Problem | Idiomatic Fix |
|-------------|---------|---------------|
| Stringly-typed APIs | No compile-time validation | Enums or newtypes |
| `unwrap()` everywhere | Panics in production | `?` with proper error types |
| `.clone()` as escape hatch | Hides ownership issues, wastes perf | Restructure borrows, use `Cow` |
| Fighting the borrow checker | Complex workarounds, fragile code | Restructure data/algorithm |
| Unnecessary `Box<dyn Trait>` | Heap allocation, vtable overhead | Generics / `impl Trait` |
| Ignoring clippy warnings | Missing idioms and bugs | Fix warnings, use `#[expect]` |
| `pub use module::*` | Unstable API, unclear docs | Explicit re-exports |
| Leaking external types | Semver coupling | Newtype wrappers |
| Excessive generics | Unreadable signatures, slow compilation | Concrete types first |
| God structs | Everything in one type | Split into focused types |
| Derived `Clone` with generic `Arc` fields | `.clone()` clones the reference, not the value | Manual `Clone` impl when `T: !Clone` |

### Stringly-Typed APIs → Enums

```rust
// ❌ DON'T: String for known set of values
fn set_log_level(level: &str) { /* "info", "debug", "error" */ }

// ✅ DO: Enum for compile-time exhaustiveness
#[derive(Debug, Clone, Copy)]
enum LogLevel { Debug, Info, Warn, Error }
fn set_log_level(level: LogLevel) { /* ... */ }
```

### Excessive unwrap → Error Propagation

```rust
// ❌ DON'T: Panic on every possible failure
fn load_config() -> Config {
    let text = std::fs::read_to_string("config.toml").unwrap();
    toml::from_str(&text).unwrap()
}

// ✅ DO: Propagate errors with context
fn load_config() -> anyhow::Result<Config> {
    let text = std::fs::read_to_string("config.toml")
        .context("failed to read config.toml")?;
    let config = toml::from_str(&text)
        .context("failed to parse config.toml")?;
    Ok(config)
}
```

### Clone as Escape Hatch → Restructure

```rust
// ❌ DON'T: Clone to avoid borrow issues
fn process(data: &mut Vec<String>) {
    let cloned = data.clone();  // Expensive!
    for item in &cloned {
        if item.starts_with("remove") {
            data.retain(|x| x != item);
        }
    }
}

// ✅ DO: Restructure to avoid the conflict
fn process(data: &mut Vec<String>) {
    data.retain(|item| !item.starts_with("remove"));
}
```

### Unnecessary Heap Allocation → Stack or Generics

```rust
// ❌ DON'T: Box<dyn Trait> when type is known
fn get_handler() -> Box<dyn Handler> { Box::new(MyHandler) }

// ✅ DO: Return concrete type or impl Trait
fn get_handler() -> impl Handler { MyHandler }

// ❌ DON'T: Vec for fixed-size data
fn get_pair() -> Vec<i32> { vec![1, 2] }

// ✅ DO: Use array or tuple
fn get_pair() -> [i32; 2] { [1, 2] }
```

### Ignoring Clippy → Fix or Expect

```rust
// ❌ DON'T: Blanket-suppress warnings
#[allow(clippy::all)]

// ✅ DO: Fix warnings, or justify exceptions
#[expect(clippy::cast_possible_truncation, reason = "value guaranteed < 256 by validation")]
let byte = value as u8;
```

### God Structs → Focused Types

```rust
// ❌ DON'T: Everything in one struct
struct App {
    config: Config,
    database: Database,
    cache: Cache,
    auth: Auth,
    mailer: Mailer,
    // ... 20 more fields
}

// ✅ DO: Compose focused types
struct App { services: Services, config: AppConfig }
struct Services { database: Database, cache: Cache, auth: Auth }
```

---

### Real-World Failures — From Production Experience

These failures come from actual Rust project development. Each caused hours of debugging.

#### Failure 1: Trusting Pre-Training Over Docs (Stale Crate API)

**What happened:** Agent used `lsp_types` crate and `#[async_trait]` macro for `tower-lsp-server` v0.23,
based on training data from the original `tower-lsp` v0.20.

**The reality:** v0.23 (community fork) migrated to `ls_types` and native async traits (edition 2024).
Every import was wrong.

```rust
// ❌ What the agent wrote (based on stale knowledge):
use lsp_types::*;
use tower_lsp::jsonrpc;
#[async_trait]
impl LanguageServer for Backend { ... }

// ✅ What actually works (tower-lsp-server v0.23):
use tower_lsp_server::ls_types::*;
use tower_lsp_server::jsonrpc;
// No #[async_trait] — native async in edition 2024
impl LanguageServer for Backend { ... }
```

**Lesson:** Always read crate documentation before implementing. If your docs say one thing
and your training data says another, trust the docs. Run `cargo doc --open -p <crate>` when unsure.
See [rule-005](../../MISTAKES.md) for the documentation-first rule.

#### Failure 2: Assuming MCP Uses LSP-Style Framing

**What happened:** Tests assumed MCP stdio transport uses Content-Length framing (like LSP does).
Test panicked with "no Content-Length header found."

**The reality:** rmcp's stdio transport uses line-delimited JSON (newline-separated), not HTTP-style
Content-Length framing.

```rust
// ❌ DON'T: Assume all protocol transports use the same framing
// write!(stdin, "Content-Length: {}\r\n\r\n{}", json.len(), json);

// ✅ DO: Check the actual transport implementation
writeln!(stdin, "{json}");  // MCP stdio = newline-delimited JSON
// vs LSP:
write!(stdin, "Content-Length: {}\r\n\r\n{}", json.len(), json);  // LSP = Content-Length
```

**Lesson:** Don't assume protocol similarity. LSP and MCP are different protocols with different
transport framing, even though both use JSON-RPC over stdio.

#### Failure 3: Arena Clone Causes SIGSEGV

**What happened:** Extracting list items from an arena-backed AST returned `Vec<ListItem>`
(owned). Cloning `ListItem` cloned its `ArenaHashMap`, which triggered a segfault because
`hashbrown::HashMap` with a `bumpalo` allocator doesn't support safe cloning.

```rust
// ❌ DON'T: Clone arena-backed types with custom allocators
fn extract_list_items(ast: &Ast) -> Vec<ListItem> {
    // This clones each ListItem, including its ArenaHashMap — SIGSEGV
    node.children().map(|c| c.clone()).collect()
}

// ✅ DO: Return references to arena-allocated items
fn extract_list_items<'a>(ast: &'a Ast) -> Vec<&'a ListItem<'a>> {
    node.children().collect()  // borrows, no clone
}
```

**Lesson:** Types backed by custom allocators (arena, pool) may not support `Clone` safely.
Return references instead of cloning. If you must own the data, convert to standard-allocator
types first.

#### Failure 4: Returning References to Stack Temporaries in Arena Code

**What happened:** `ListItem::from_node()` used `&[]` (literal empty slice) for the `children_list`
field. This creates a reference to a stack-local temporary that's freed when the function returns.

```rust
// ❌ DON'T: Return reference to stack temporary
fn from_node(node: &Node, arena: &'arena Bump) -> ListItem<'arena> {
    ListItem {
        children_list: &[],  // dangling reference after function returns!
        // ...
    }
}

// ✅ DO: Allocate the empty slice in the arena
fn from_node(node: &Node, arena: &'arena Bump) -> ListItem<'arena> {
    ListItem {
        children_list: bumpalo::collections::Vec::new_in(arena).into_bump_slice(),
        // ...
    }
}
```

**Lesson:** When a struct has `&'arena [T]` fields, every value — including empty slices —
should be allocated from the arena. Literal `&[]` has an implicit `'static` lifetime via
const promotion, which can mask lifetime mismatches in arena-backed types.

#### Failure 5: Silently Overwriting Duplicate Keys in Index

**What happened:** `RealmIndex::add_document()` used `HashMap::insert()` for block IDs.
When two documents had the same block ID (`^my-block`), the second silently overwrote the first,
dropping cross-document references.

```rust
// ❌ DON'T: Silently overwrite on collision
fn add_document(&mut self, doc: &DocumentIndex) {
    for block in &doc.blocks {
        self.block_map.insert(block.id.clone(), block.entry());
        // Second insert silently drops the first document's block!
    }
}

// ✅ DO: Store all occurrences, return first on lookup
fn add_document(&mut self, doc: &DocumentIndex) {
    for block in &doc.blocks {
        self.block_map
            .entry(block.id.clone())
            .or_default()
            .push(block.entry());
    }
}

fn lookup_block(&self, id: &str) -> Option<&BlockEntry> {
    self.block_map.get(id)?.first()  // first-in semantics
}
```

**Lesson:** When building an index from multiple documents, duplicates are expected.
Use `Vec` values in the map to preserve all occurrences. Choose explicit resolution
semantics (first-in, last-in, error) rather than silent overwrite.

#### Failure 6: Semver Mismatch with Transitive Dependencies

**What happened:** Added `schemars = "0.8"` for `rmcp`'s `JsonSchema` derives. But `rmcp`
internally uses `schemars = "1.x"`. Two incompatible versions of the same crate caused derive
bound failures.

```toml
# ❌ DON'T: Guess dependency versions
[dependencies]
schemars = "0.8"  # Wrong — rmcp needs 1.x

# ✅ DO: Check what your dependencies expect
# Run: cargo tree -p schemars
# Or: cargo tree -i schemars
[dependencies]
schemars = "1.2"  # Matches rmcp's expectation
```

**Lesson:** Before adding a dependency that's also used transitively, check `cargo tree`
for version expectations. When two crates need different major versions of the same
dependency, you get two copies and incompatible types.

#### Gotcha: Derived Clone with Generic Arc Fields

When `#[derive(Clone)]` is used on a generic struct, the generated impl adds `T: Clone`
as a bound — even when `Clone` doesn't need `T: Clone` (e.g., `Arc<T>` is always `Clone`).

```rust
#[derive(Clone)]
struct Container<T>(Arc<T>);

// Generated code (roughly):
// impl<T> Clone for Container<T> where T: Clone { ... }

fn example<T>(c: &Container<T>) {
    let cloned = c.clone();
    // If T: Clone → cloned: Container<T> (correct)
    // If T: !Clone → cloned: &Container<T> (autoref! Just clones the reference)
}
```

When `T: !Clone`, Rust's method resolution tries `Container<T>::clone()` (fails),
then falls back to `(&Container<T>)::clone()` (succeeds — references are always `Clone`).
The result is a reference, not a cloned container.

```rust
// ✅ FIX: Manual Clone impl without T: Clone bound
impl<T> Clone for Container<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
```

**Rule:** If your struct uses `Arc<T>`, `Rc<T>`, or other wrappers that are `Clone`
regardless of `T`, write a manual `Clone` impl instead of deriving.

### References

- Clippy lints: [rust-lang.github.io/rust-clippy](https://rust-lang.github.io/rust-clippy/)
- Guidelines: [M-CONCISE-NAMES](../../docs/rust_guidelines/universal.md), [M-STATIC-VERIFICATION](../../docs/rust_guidelines/universal.md)
- Related: [core/errors.md](../core/errors.md), [core/ownership.md](../core/ownership.md), [patterns/idioms.md](idioms.md)
- Cross-cutting: [async-ready.md](async-ready.md) (Send/Sync issues), [cookbook.md](cookbook.md) (complete examples)
