## Testing — Unit, Integration, Doc & Property Tests

> **TL;DR:** Unit tests go in the same file with `#[cfg(test)]`. Integration tests go in
> `tests/`. Doc tests verify examples compile and work. Use `proptest` for property testing.
> Gate test utilities behind `test-util` feature.

### Unit Tests

```rust
// Same file as the code being tested
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_overflow() {
        add(i32::MAX, 1);  // panics
    }
}
```

### Integration Tests

```
tests/
├── common/
│   └── mod.rs          ← Shared helpers (not a test file)
├── api_tests.rs        ← Each file is a separate test binary
└── storage_tests.rs
```

```rust
// tests/api_tests.rs
mod common;

use my_crate::Server;

#[test]
fn test_server_starts() {
    let server = Server::new(common::test_config());
    assert!(server.is_running());
}
```

### Doc Tests

```rust
/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// use my_crate::add;
/// assert_eq!(add(2, 3), 5);
/// ```
///
/// ```should_panic
/// use my_crate::divide;
/// divide(1, 0);  // panics on division by zero
/// ```
pub fn add(a: i32, b: i32) -> i32 { a + b }
```

Doc test markers: ` ```rust ` (default, must compile+run), ` ```ignore ` (skip), ` ```compile_fail ` (must fail to compile), ` ```should_panic ` (must panic), ` ```no_run ` (compile but don't run).

### Property Testing with proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_sort_preserves_length(ref v in prop::collection::vec(any::<i32>(), 0..100)) {
        let mut sorted = v.clone();
        sorted.sort();
        prop_assert_eq!(v.len(), sorted.len());
    }

    #[test]
    fn test_parse_roundtrip(s in "[a-zA-Z0-9]+") {
        let parsed = MyType::parse(&s);
        if let Ok(val) = parsed {
            prop_assert_eq!(val.to_string(), s);
        }
    }
}
```

### Test Utilities Feature Gate

```toml
[features]
test-util = []  # Gate test helpers

# In lib.rs:
```

```rust
#[cfg(feature = "test-util")]
pub mod test_helpers {
    pub fn mock_config() -> super::Config {
        super::Config::default()
    }
}
```

### Useful Test Commands

| Command | Purpose |
|---------|---------|
| `cargo test` | Run all tests |
| `cargo test test_name` | Run matching tests |
| `cargo test -- --nocapture` | Show println output |
| `cargo test --doc` | Doc tests only |
| `cargo test --test integration` | Specific integration test |
| `cargo test --no-default-features` | Test without defaults |

### References

- The Rust Book: [Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- Guidelines: [M-TEST-UTIL](../../docs/rust_guidelines/libraries-resilience.md), [M-MOCKABLE-SYSCALLS](../../docs/rust_guidelines/libraries-resilience.md)
- Related: [tooling/documentation.md](documentation.md) (doc tests)
