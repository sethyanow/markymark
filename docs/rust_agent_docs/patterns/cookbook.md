## Cookbook — Complete Working Recipes

> **TL;DR:** End-to-end examples combining multiple Rust concepts. Each recipe is
> self-contained, compilable, and demonstrates real patterns you'll use in production code.

### Recipe 1: Parse and Validate a TOML Config

**Concepts:** serde, error handling, builder pattern, validation

```rust
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RawConfig {
    host: Option<String>,
    port: Option<u16>,
    max_connections: Option<usize>,
    database_url: String,
}

#[derive(Debug)]
struct Config {
    host: String,
    port: u16,
    max_connections: usize,
    database_url: String,
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("validation failed: {0}")]
    Validation(String),
}

impl Config {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        let raw: RawConfig = toml::from_str(&text)?;

        let config = Config {
            host: raw.host.unwrap_or_else(|| "127.0.0.1".into()),
            port: raw.port.unwrap_or(8080),
            max_connections: raw.max_connections.unwrap_or(100),
            database_url: raw.database_url,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::Validation("port cannot be 0".into()));
        }
        if self.database_url.is_empty() {
            return Err(ConfigError::Validation("database_url is required".into()));
        }
        if self.max_connections == 0 {
            return Err(ConfigError::Validation(
                "max_connections must be positive".into(),
            ));
        }
        Ok(())
    }
}
```

**Key patterns:** Separate raw (deserialized) from validated types. Defaults via `unwrap_or`.
Validation returns `Result`, never panics. `thiserror` for library-quality errors.

---

### Recipe 2: Newtype with Full Trait Suite

**Concepts:** newtype, derive, Display, FromStr, Deref, serde, validation

```rust
use std::fmt;
use std::str::FromStr;

/// A validated email address. Guaranteed non-empty and contains '@'.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

#[derive(Debug, thiserror::Error)]
#[error("invalid email: {0}")]
pub struct EmailError(String);

impl Email {
    pub fn new(value: impl Into<String>) -> Result<Self, EmailError> {
        let s = value.into();
        if s.contains('@') && s.len() > 3 {
            Ok(Email(s))
        } else {
            Err(EmailError(format!("'{s}' is not a valid email")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Email {
    type Err = EmailError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Email::new(s)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Usage:
// let email: Email = "user@example.com".parse()?;
// println!("Sending to {email}");  // Display
// some_fn(email.as_ref());         // AsRef<str>
```

**Key patterns:** Constructor validates invariants — invalid `Email` can never exist.
`FromStr` enables `.parse()`. `AsRef<str>` enables passing to string-accepting functions.
Don't implement `Deref<Target = str>` — newtypes should not transparently coerce.

---

### Recipe 3: Trait Object Service with Dynamic Dispatch

**Concepts:** trait objects, Box<dyn Trait>, async, error handling, Send + Sync

```rust
use std::collections::HashMap;

/// A pluggable storage backend.
pub trait Storage: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError>;
    fn set(&mut self, key: &str, value: String) -> Result<(), StorageError>;
    fn delete(&mut self, key: &str) -> Result<bool, StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("key not found: {0}")]
    NotFound(String),
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}

/// In-memory implementation for testing.
pub struct MemoryStorage {
    data: HashMap<String, String>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }
}

impl Storage for MemoryStorage {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self.data.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: String) -> Result<(), StorageError> {
        self.data.insert(key.to_owned(), value);
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<bool, StorageError> {
        Ok(self.data.remove(key).is_some())
    }
}

/// Service that works with any Storage implementation.
pub struct Service {
    storage: Box<dyn Storage>,
}

impl Service {
    pub fn new(storage: impl Storage + 'static) -> Self {
        Self { storage: Box::new(storage) }
    }

    pub fn process(&mut self, key: &str) -> Result<String, StorageError> {
        match self.storage.get(key)? {
            Some(val) => Ok(val),
            None => {
                let default = "initialized".to_owned();
                self.storage.set(key, default.clone())?;
                Ok(default)
            }
        }
    }
}
```

**Key patterns:** `Send + Sync` bounds on trait enable use in async contexts.
`Box<dyn Storage>` for runtime polymorphism. `impl Storage + 'static` in constructor
accepts any concrete type. In-memory implementation for testing, real implementation for production.

---

### Recipe 4: Iterator Adapter Chain with Custom Collector

**Concepts:** iterators, closures, FromIterator, method chaining, zero-cost abstractions

```rust
use std::collections::HashMap;

#[derive(Debug)]
struct LogEntry {
    level: String,
    message: String,
    timestamp: u64,
}

/// Parse log lines into structured entries, filter, and group by level.
fn analyze_logs(raw_lines: &[&str]) -> HashMap<String, Vec<LogEntry>> {
    raw_lines
        .iter()
        .filter_map(|line| parse_log_line(line))  // skip unparseable lines
        .filter(|entry| entry.timestamp > 1000)    // filter old entries
        .fold(HashMap::new(), |mut groups, entry| {
            groups
                .entry(entry.level.clone())
                .or_default()
                .push(entry);
            groups
        })
}

fn parse_log_line(line: &str) -> Option<LogEntry> {
    // Format: "TIMESTAMP LEVEL message text"
    let mut parts = line.splitn(3, ' ');
    let timestamp: u64 = parts.next()?.parse().ok()?;
    let level = parts.next()?.to_owned();
    let message = parts.next()?.to_owned();
    Some(LogEntry { level, message, timestamp })
}

/// Custom iterator: windows of N elements with step size
struct SteppedWindows<'a, T> {
    data: &'a [T],
    window: usize,
    step: usize,
    pos: usize,
}

impl<'a, T> SteppedWindows<'a, T> {
    fn new(data: &'a [T], window: usize, step: usize) -> Self {
        Self { data, window, step, pos: 0 }
    }
}

impl<'a, T> Iterator for SteppedWindows<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + self.window <= self.data.len() {
            let slice = &self.data[self.pos..self.pos + self.window];
            self.pos += self.step;
            Some(slice)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.len().saturating_sub(self.pos + self.window - 1);
        let count = (remaining + self.step - 1) / self.step;
        (count, Some(count))
    }
}
```

**Key patterns:** `filter_map` for parse-and-filter in one step. `fold` to accumulate
into a HashMap. Custom iterators implement `Iterator` trait with `size_hint` for
allocation optimization. Lifetime `'a` ties iterator to data source.

---

### Recipe 5: Thread-Safe Shared State with Arc + Mutex

**Concepts:** Arc, Mutex, Clone for sharing, scoped access, avoiding lock poisoning

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<StateInner>>,
}

struct StateInner {
    counters: HashMap<String, u64>,
    total_requests: u64,
}

impl AppState {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(StateInner {
                counters: HashMap::new(),
                total_requests: 0,
            })),
        }
    }

    /// Increment a named counter. Returns the new value.
    fn increment(&self, name: &str) -> u64 {
        let mut state = self.inner.lock().expect("lock poisoned");
        state.total_requests += 1;
        let counter = state.counters.entry(name.to_owned()).or_insert(0);
        *counter += 1;
        *counter
    }

    /// Get a snapshot of all counters (releases lock immediately).
    fn snapshot(&self) -> HashMap<String, u64> {
        let state = self.inner.lock().expect("lock poisoned");
        state.counters.clone()
    }

    fn total_requests(&self) -> u64 {
        self.inner.lock().expect("lock poisoned").total_requests
    }
}

// Usage with multiple threads:
// let state = AppState::new();
// let handles: Vec<_> = (0..4).map(|i| {
//     let state = state.clone();  // Arc::clone is cheap
//     std::thread::spawn(move || {
//         for _ in 0..100 {
//             state.increment(&format!("thread-{i}"));
//         }
//     })
// }).collect();
// for h in handles { h.join().unwrap(); }
// assert_eq!(state.total_requests(), 400);
```

**Key patterns:** `Arc<Mutex<T>>` split into outer handle (`AppState`) and inner data
(`StateInner`). `#[derive(Clone)]` on the handle clones the `Arc`, not the data.
Lock scope minimized — acquire, mutate, release. `snapshot()` clones data out of
the lock so callers can read without holding it.

---

### Recipe Selection Guide

```
What are you building?
├─ Config parsing with validation?
│   └─ Recipe 1 (serde + thiserror + validation)
├─ Domain type with validation invariants?
│   └─ Recipe 2 (newtype + FromStr + Display)
├─ Pluggable backend or strategy pattern?
│   └─ Recipe 3 (trait object + Box<dyn Trait>)
├─ Data transformation pipeline?
│   └─ Recipe 4 (iterator chains + custom Iterator)
├─ Shared mutable state across threads?
│   └─ Recipe 5 (Arc + Mutex + handle pattern)
└─ Combining several?
    └─ Read all relevant recipes, compose the patterns
```

### References

- Related: [idioms.md](idioms.md) (individual patterns), [api-design.md](api-design.md) (public APIs)
- Related: [../core/errors.md](../core/errors.md) (error handling), [../core/traits.md](../core/traits.md) (trait design)
- Related: [../core/closures.md](../core/closures.md) (closure patterns), [../advanced/concurrency.md](../advanced/concurrency.md) (Send/Sync)
