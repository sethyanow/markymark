# Error Handling - thiserror & anyhow

<agent>
<goal>Design error types with thiserror (libraries) and handle errors with anyhow (applications).</goal>
<when_to_use>When defining custom error types or handling errors in Rust code.</when_to_use>
<contains>thiserror derive macros, anyhow context, error conversion, when to use each</contains>
<see_also>tower-lsp.md</see_also>
</agent>

**TL;DR:** Use `thiserror` for library error types (structured, typed). Use `anyhow` for application error handling (convenient, contextual). Never use `anyhow::Error` in library public APIs.

**Checklist:**
- [ ] Library code: `thiserror` with `#[derive(Error)]`
- [ ] Application code: `anyhow::Result` and `.context()`
- [ ] Convert between them at boundaries
- [ ] Use `#[from]` for automatic conversions
- [ ] Add context with `.context()` or `.with_context()`

---

## Setup

### Cargo.toml

```toml
[dependencies]
thiserror = "1"  # For libraries
anyhow = "1"     # For applications
```

---

## thiserror - Library Errors

### Basic Error Enum

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid syntax at line {line}: {message}")]
    InvalidSyntax { line: usize, message: String },

    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("maximum nesting depth exceeded")]
    MaxDepthExceeded,
}

// Usage
fn parse_document(input: &str) -> Result<Document, ParseError> {
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof);
    }
    // ...
    Ok(Document::default())
}
```

### Wrapping Other Errors

```rust
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("failed to read file: {path}")]
    ReadError {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("parse error")]
    ParseError(#[from] ParseError),  // Auto-convert ParseError

    #[error("invalid UTF-8 in file")]
    Utf8Error(#[from] std::str::Utf8Error),
}

// With #[from], this works automatically:
fn load_document(path: &str) -> Result<Document, DocumentError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| DocumentError::ReadError {
            path: path.to_string(),
            source: e,
        })?;

    let doc = parse_document(&content)?;  // ParseError auto-converts
    Ok(doc)
}
```

### Error with Backtrace

```rust
use thiserror::Error;
use std::backtrace::Backtrace;

#[derive(Error, Debug)]
pub enum CriticalError {
    #[error("internal error: {message}")]
    Internal {
        message: String,
        backtrace: Backtrace,
    },
}

impl CriticalError {
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            backtrace: Backtrace::capture(),
        }
    }
}
```

### Transparent Wrapper

```rust
use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]  // Display delegates to inner error
pub struct LspError(#[from] Box<dyn std::error::Error + Send + Sync>);

// Useful for type erasure while maintaining error trait
```

---

## anyhow - Application Errors

### Basic Usage

```rust
use anyhow::{Result, Context, bail, ensure};

fn process_file(path: &str) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .context("failed to read input file")?;

    let doc = parse(&content)
        .with_context(|| format!("failed to parse {}", path))?;

    validate(&doc)?;

    Ok(())
}

fn main() -> Result<()> {
    process_file("input.md")?;
    println!("Success!");
    Ok(())
}
```

### Adding Context

```rust
use anyhow::{Result, Context};

fn load_config() -> Result<Config> {
    let path = find_config_path()
        .context("config file not found")?;

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("invalid TOML in {}", path.display()))?;

    Ok(config)
}

// Error chain:
// "invalid TOML in /home/user/.config/app.toml"
// Caused by:
//   "expected '=' at line 5"
```

### bail! and ensure!

```rust
use anyhow::{Result, bail, ensure};

fn validate_document(doc: &Document) -> Result<()> {
    // bail! returns early with error
    if doc.title.is_empty() {
        bail!("document must have a title");
    }

    // ensure! is like assert but returns Result
    ensure!(
        doc.headings.len() > 0,
        "document must have at least one heading"
    );

    ensure!(
        doc.word_count < 10_000,
        "document too long: {} words (max 10000)",
        doc.word_count
    );

    Ok(())
}
```

### Downcasting Errors

```rust
use anyhow::{Result, Error};

fn handle_error(err: Error) {
    // Try to downcast to specific error type
    if let Some(parse_err) = err.downcast_ref::<ParseError>() {
        match parse_err {
            ParseError::InvalidSyntax { line, .. } => {
                eprintln!("Syntax error on line {}", line);
            }
            _ => eprintln!("Parse error: {}", parse_err),
        }
    } else if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        eprintln!("IO error: {}", io_err);
    } else {
        eprintln!("Error: {}", err);
    }

    // Print full error chain
    eprintln!("\nFull error chain:");
    for (i, cause) in err.chain().enumerate() {
        eprintln!("  {}: {}", i, cause);
    }
}
```

---

## Patterns

### Library/Application Boundary

```rust
// In library (markymark-core)
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RealmError {
    #[error("realm not found: {0}")]
    NotFound(String),

    #[error("realm already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid realm configuration")]
    InvalidConfig(#[source] ConfigError),
}

// In application (markymark-cli)
use anyhow::{Result, Context};
use markymark_core::RealmError;

fn create_realm(name: &str) -> Result<()> {
    markymark_core::create_realm(name)
        .context("failed to create realm")?;

    println!("Created realm: {}", name);
    Ok(())
}

fn main() {
    if let Err(e) = create_realm("my-realm") {
        // Rich error output with chain
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}
```

### LSP Error Handling

```rust
use thiserror::Error;
use tower_lsp::jsonrpc;

#[derive(Error, Debug)]
pub enum LspHandlerError {
    #[error("document not found: {0}")]
    DocumentNotFound(String),

    #[error("invalid position: line {line}, character {character}")]
    InvalidPosition { line: u32, character: u32 },

    #[error("operation not supported")]
    NotSupported,

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<LspHandlerError> for jsonrpc::Error {
    fn from(err: LspHandlerError) -> Self {
        match err {
            LspHandlerError::DocumentNotFound(uri) => {
                jsonrpc::Error {
                    code: jsonrpc::ErrorCode::InvalidParams,
                    message: format!("Document not found: {}", uri).into(),
                    data: None,
                }
            }
            LspHandlerError::InvalidPosition { line, character } => {
                jsonrpc::Error {
                    code: jsonrpc::ErrorCode::InvalidParams,
                    message: format!("Invalid position: {}:{}", line, character).into(),
                    data: None,
                }
            }
            LspHandlerError::NotSupported => {
                jsonrpc::Error {
                    code: jsonrpc::ErrorCode::MethodNotFound,
                    message: "Operation not supported".into(),
                    data: None,
                }
            }
            LspHandlerError::Internal(msg) => {
                jsonrpc::Error {
                    code: jsonrpc::ErrorCode::InternalError,
                    message: msg.into(),
                    data: None,
                }
            }
        }
    }
}

// In handler
async fn goto_definition(&self, params: GotoDefinitionParams) -> jsonrpc::Result<Option<Location>> {
    let result = self.inner_goto_definition(params).await
        .map_err(|e: LspHandlerError| -> jsonrpc::Error { e.into() })?;
    Ok(result)
}
```

### Result Type Aliases

```rust
// In library
pub type ParseResult<T> = Result<T, ParseError>;
pub type RealmResult<T> = Result<T, RealmError>;

// Usage
fn parse_heading(input: &str) -> ParseResult<Heading> {
    // ...
}

// In application, just use anyhow::Result<T>
```

### Combining Multiple Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("parse error")]
    Parse(#[from] ParseError),

    #[error("graph error")]
    Graph(#[from] GraphError),

    #[error("IO error")]
    Io(#[from] std::io::Error),
}

// Now functions can return IndexError and use ? on all three types
fn build_index(path: &str) -> Result<Index, IndexError> {
    let content = std::fs::read_to_string(path)?;  // io::Error -> IndexError
    let doc = parse(&content)?;                     // ParseError -> IndexError
    let graph = build_graph(&doc)?;                 // GraphError -> IndexError
    Ok(Index { doc, graph })
}
```

---

## Pitfalls

### anyhow in Library Public API

<pitfall>
**Problem:** Using `anyhow::Error` in library public API prevents callers from matching on error types.

```rust
// BAD: Library function returns anyhow::Result
pub fn parse_document(input: &str) -> anyhow::Result<Document> {
    // Caller can't match on specific errors!
}
```

**Solution:** Use thiserror for library APIs:

```rust
// GOOD: Library uses thiserror
pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    // Caller can match on ParseError variants
}
```
</pitfall>

### Missing #[from] Conversions

<pitfall>
**Problem:** Using `?` without `#[from]` causes compile error.

```rust
#[derive(Error, Debug)]
pub enum MyError {
    #[error("io error")]
    Io(std::io::Error),  // Missing #[from]!
}

fn read_file() -> Result<String, MyError> {
    let s = std::fs::read_to_string("file")?;  // ERROR: can't convert
    Ok(s)
}
```

**Solution:** Add `#[from]` or convert manually:

```rust
// Option 1: Add #[from]
#[derive(Error, Debug)]
pub enum MyError {
    #[error("io error")]
    Io(#[from] std::io::Error),
}

// Option 2: Manual conversion
fn read_file() -> Result<String, MyError> {
    let s = std::fs::read_to_string("file")
        .map_err(MyError::Io)?;
    Ok(s)
}
```
</pitfall>

### Lost Context with ?

<pitfall>
**Problem:** Plain `?` loses context about what failed.

```rust
// BAD: If this fails, you just know "IO error" but not which file
fn process() -> Result<()> {
    let a = std::fs::read_to_string("file_a.txt")?;
    let b = std::fs::read_to_string("file_b.txt")?;
    Ok(())
}
```

**Solution:** Add context:

```rust
// GOOD: Context tells you which file failed
fn process() -> Result<()> {
    let a = std::fs::read_to_string("file_a.txt")
        .context("failed to read file_a.txt")?;
    let b = std::fs::read_to_string("file_b.txt")
        .context("failed to read file_b.txt")?;
    Ok(())
}
```
</pitfall>

### Error Display vs Debug

<pitfall>
**Problem:** `println!("{}", err)` only shows top-level message, not chain.

```rust
// Shows only: "failed to process document"
println!("Error: {}", err);
```

**Solution:** Use `{:?}` or iterate chain:

```rust
// Shows full chain with {:?}
println!("Error: {:?}", err);

// Or manually iterate
for cause in err.chain() {
    println!("  Caused by: {}", cause);
}
```
</pitfall>

---

## Related

- LSP error responses: `tower-lsp.md`
- thiserror docs: https://docs.rs/thiserror/
- anyhow docs: https://docs.rs/anyhow/
- Error Handling Working Group: https://blog.rust-lang.org/inside-rust/2020/11/23/What-the-error-handling-project-group-is-working-on.html
