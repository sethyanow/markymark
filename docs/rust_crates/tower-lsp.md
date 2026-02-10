# tower-lsp-server - LSP Server Framework

<agent>
<goal>Build async LSP servers with the community fork tower-lsp-server, with proper state management and capability negotiation.</goal>
<when_to_use>When implementing Language Server Protocol servers in Rust.</when_to_use>
<contains>Server setup, LanguageServer trait, state patterns, capability config, notifications, custom methods</contains>
<see_also>tree-sitter.md, petgraph.md, error-handling.md</see_also>
</agent>

**TL;DR:** tower-lsp-server provides async LSP server infrastructure. Implement `LanguageServer` trait (native async — no `#[async_trait]`), use `Client` for notifications, manage state with interior mutability.

**Note:** This documents `tower-lsp-server` (the [community fork](https://github.com/tower-lsp-community/tower-lsp-server)), not the original `tower-lsp`. The community fork is actively maintained (v0.23+) and used by Biome, Oxc, and Veryl. The original `tower-lsp` has not been updated since August 2023.

**Critical differences from original tower-lsp:**
- **Types**: Re-exports `ls_types` (NOT `lsp_types`). Use `tower_lsp_server::ls_types::*`.
- **Async**: Uses native async traits (Rust 1.75+ RPITIT). No `#[async_trait]` macro needed.
- **URI**: `ls_types::Uri` uses `fluent_uri` (created via `.parse::<Uri>()`), not `url::Url`.
- **Service**: `LspService::new(Backend::new)` takes a function pointer, not a closure.

**Checklist:**
- [ ] Use `tower_lsp_server::ls_types::*` (NOT `lsp_types`)
- [ ] Implement `LanguageServer` with plain `async fn` (no async_trait macro)
- [ ] Use `ls_types::Uri` (fluent_uri-based, FromStr) not `url::Url`
- [ ] `LspService::new(Backend::new)` with fn pointer
- [ ] Use `Client` for sending notifications/requests to editor
- [ ] Declare capabilities in `initialize` response
- [ ] Handle state with `tokio::sync::RwLock` or similar
- [ ] Use `jsonrpc::Result` for error responses

---

## Setup

### Cargo.toml

```toml
[dependencies]
tower-lsp-server = "0.23"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Basic Server Structure

```rust
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tokio::sync::RwLock;
use std::sync::Arc;

struct Backend {
    client: Client,
    state: Arc<RwLock<ServerState>>,
}

#[derive(Default)]
struct ServerState {
    documents: HashMap<Uri, String>,
    // ... other state
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ServerState::default())),
        }
    }
}

// NOTE: No #[async_trait] needed — tower-lsp-server v0.23 uses native async traits
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    }
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // NOTE: Function pointer, not closure
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

---

## Patterns

### Document Synchronization

```rust
// NOTE: No #[async_trait] — native async
impl LanguageServer for Backend {
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let mut state = self.state.write().await;
        state.documents.insert(uri.clone(), text.clone());

        // Trigger diagnostics
        drop(state); // Release lock before async call
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        let mut state = self.state.write().await;
        if let Some(doc) = state.documents.get_mut(&uri) {
            // For FULL sync: take the last change (entire document)
            if let Some(change) = params.content_changes.into_iter().last() {
                *doc = change.text;
            }
        }

        drop(state);
        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.write().await;
        state.documents.remove(&params.text_document.uri);
    }
}
```

### Go to Definition

```rust
async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let state = self.state.read().await;
    let doc = state.documents.get(&uri).ok_or_else(|| {
        jsonrpc::Error::invalid_params("document not found")
    })?;

    // Find symbol at position, resolve to definition
    if let Some(def_location) = find_definition(doc, position) {
        Ok(Some(GotoDefinitionResponse::Scalar(def_location)))
    } else {
        Ok(None)
    }
}
```

### Find References

```rust
async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_declaration = params.context.include_declaration;

    let state = self.state.read().await;

    // Find symbol at position
    let symbol = find_symbol_at(&state, &uri, position)?;

    // Collect all references
    let mut refs = find_all_references(&state, &symbol);

    if !include_declaration {
        refs.retain(|loc| !is_declaration(loc, &symbol));
    }

    Ok(if refs.is_empty() { None } else { Some(refs) })
}
```

### Rename with Prepare

```rust
async fn prepare_rename(
    &self,
    params: TextDocumentPositionParams,
) -> Result<Option<PrepareRenameResponse>> {
    let uri = params.text_document.uri;
    let position = params.position;

    let state = self.state.read().await;

    // Check if rename is valid at position
    if let Some((range, placeholder)) = can_rename_at(&state, &uri, position) {
        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder,
        }))
    } else {
        Ok(None) // Cannot rename here
    }
}

async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = params.new_name;

    let state = self.state.read().await;

    // Find symbol and all references
    let symbol = find_symbol_at(&state, &uri, position)?;
    let refs = find_all_references(&state, &symbol);

    // Build workspace edit
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

    for location in refs {
        changes
            .entry(location.uri.clone())
            .or_default()
            .push(TextEdit {
                range: location.range,
                new_text: new_name.clone(),
            });
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }))
}
```

### Publishing Diagnostics

```rust
impl Backend {
    async fn publish_diagnostics(&self, uri: Uri) {
        let state = self.state.read().await;
        let Some(doc) = state.documents.get(&uri) else { return };

        let diagnostics = compute_diagnostics(doc);

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

fn compute_diagnostics(doc: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Example: find broken links
    for link in find_links(doc) {
        if !link.is_resolved() {
            diagnostics.push(Diagnostic {
                range: link.range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("broken-link".into())),
                source: Some("markymark".into()),
                message: format!("Broken link: {}", link.target),
                ..Default::default()
            });
        }
    }

    diagnostics
}
```

### Custom Methods

```rust
// In your LanguageServer impl, add custom method handlers
impl Backend {
    async fn create_realm(&self, params: CreateRealmParams) -> Result<RealmId> {
        let mut state = self.state.write().await;
        let id = state.create_realm(params.config)?;
        Ok(id)
    }
}

// Register custom methods in main
let (service, socket) = LspService::build(Backend::new)
    .custom_method("markymark/createRealm", Backend::create_realm)
    .custom_method("markymark/destroyRealm", Backend::destroy_realm)
    .finish();
```

### URI Handling

```rust
// ls_types::Uri uses fluent_uri (NOT url::Url)
// Create URIs via FromStr / .parse()
let uri: ls_types::Uri = "file:///path/to/doc.md".parse().unwrap();

// Convert from string
fn string_to_uri(s: &str) -> Result<ls_types::Uri, String> {
    s.parse::<ls_types::Uri>().map_err(|e| e.to_string())
}

// Get string representation
let uri_str: &str = uri.as_str();
```

### Creating the Service

```rust
// Function pointer — NOT a closure
let (service, socket) = LspService::new(Backend::new);

// With custom methods — also fn pointer
let (service, socket) = LspService::build(Backend::new)
    .custom_method("custom/method", Backend::handler)
    .finish();

// Access the backend from service (useful in tests)
let backend = service.inner();
```

---

## Pitfalls

### ls_types vs lsp_types — CRITICAL

<pitfall>
**Problem:** Using `lsp_types` crate directly or importing `tower_lsp_server::lsp_types::*` fails to compile. Old tower-lsp (v0.20) used `lsp_types`; the community fork tower-lsp-server (v0.23) re-exports `ls_types` instead.

```rust
// WRONG — does not exist in tower-lsp-server v0.23
use tower_lsp_server::lsp_types::*;
use lsp_types::*;

// WRONG — Url is from url crate, not ls_types
use url::Url;
```

**Solution:** Always use `ls_types` from tower-lsp-server's re-export:

```rust
// CORRECT
use tower_lsp_server::ls_types::*;

// URI is ls_types::Uri (fluent_uri-based), created via .parse()
let uri: Uri = "file:///path".parse().unwrap();
```

**Note:** Your Cargo.toml may still list `lsp-types` as a workspace dependency for other uses, but the LSP server code must use `ls_types`.
</pitfall>

### No async_trait Macro Needed

<pitfall>
**Problem:** Adding `#[tower_lsp_server::async_trait]` or `#[async_trait::async_trait]` causes compilation errors. Old tower-lsp required this; the community fork uses native async traits (RPITIT, stabilized in Rust 1.75+).

```rust
// WRONG — don't use async_trait macro
#[tower_lsp_server::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> { ... }
}
```

**Solution:** Plain `impl` block with `async fn`:

```rust
// CORRECT — native async, no macro
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> { ... }
}
```

This works with edition 2021 on Rust 1.75+. No need for edition 2024.
</pitfall>

### Lock Ordering — Deadlock Risk

<pitfall>
**Problem:** Holding a lock while making async calls can cause deadlocks.

```rust
// BAD: Holding lock across await
async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let mut state = self.state.write().await;
    state.documents.insert(uri, text);
    self.publish_diagnostics(uri).await; // Still holding lock!
}
```

**Solution:** Drop locks before async calls.

```rust
// GOOD: Release lock before await
async fn did_change(&self, params: DidChangeTextDocumentParams) {
    {
        let mut state = self.state.write().await;
        state.documents.insert(uri.clone(), text);
    } // Lock released here
    self.publish_diagnostics(uri).await;
}
```

**markymark note:** All notification handlers (did_open, did_change, did_close) do synchronous state mutation only — the lock drops at function end. No async calls while holding the lock.
</pitfall>

### TextDocumentSyncKind Mismatch

<pitfall>
**Problem:** Declaring `INCREMENTAL` sync but handling as `FULL` causes corruption.

**Solution:** Match your `did_change` implementation to declared capability:

```rust
// FULL sync (simpler — recommended for v1):
TextDocumentSyncKind::FULL
// Take last change = entire document text
if let Some(change) = params.content_changes.into_iter().last() {
    *doc = change.text;
}

// INCREMENTAL sync (more complex, better perf for large docs):
TextDocumentSyncKind::INCREMENTAL
for change in params.content_changes {
    if let Some(range) = change.range {
        apply_incremental_change(doc, range, &change.text);
    } else {
        *doc = change.text;
    }
}
```

**markymark note:** Uses FULL sync for simplicity. Can upgrade to INCREMENTAL later if perf requires.
</pitfall>

### Position Encoding

<pitfall>
**Problem:** LSP uses UTF-16 code units by default, Rust strings are UTF-8.

**Solution:** Always convert positions:

```rust
fn lsp_position_to_offset(text: &str, position: Position) -> usize {
    let mut offset = 0;
    for (line_idx, line) in text.lines().enumerate() {
        if line_idx == position.line as usize {
            // Convert UTF-16 character offset to UTF-8 byte offset
            let mut utf16_offset = 0;
            for (byte_idx, ch) in line.char_indices() {
                if utf16_offset >= position.character as usize {
                    return offset + byte_idx;
                }
                utf16_offset += ch.len_utf16();
            }
            return offset + line.len();
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}
```

Or negotiate UTF-8 position encoding in capabilities:

```rust
InitializeResult {
    capabilities: ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF8),
        ..
    },
}
```
</pitfall>

### Client Not Ready

<pitfall>
**Problem:** Sending notifications before `initialized` is received.

**Solution:** Track initialization state:

```rust
struct Backend {
    client: Client,
    initialized: AtomicBool,
}

async fn initialized(&self, _: InitializedParams) {
    self.initialized.store(true, Ordering::SeqCst);
}

async fn publish_diagnostics(&self, uri: Uri) {
    if !self.initialized.load(Ordering::SeqCst) {
        return; // Client not ready
    }
    // ...
}
```
</pitfall>

### LspService::new Takes Function Pointer

<pitfall>
**Problem:** Passing a closure to `LspService::new` fails to compile.

```rust
// WRONG — closure doesn't match expected signature
let (service, socket) = LspService::new(|client| Backend::new(client));
```

**Solution:** Pass a function pointer directly:

```rust
// CORRECT
let (service, socket) = LspService::new(Backend::new);
```

This requires `Backend::new` to have the exact signature `fn(Client) -> Backend`.
</pitfall>

---

## Migration from tower-lsp (original)

If porting from `tower-lsp` v0.20 to `tower-lsp-server` v0.23:

| Original (tower-lsp v0.20) | Community fork (tower-lsp-server v0.23) |
|---|---|
| `tower_lsp::lsp_types::*` | `tower_lsp_server::ls_types::*` |
| `#[tower_lsp::async_trait]` | Remove — native async |
| `url::Url` for URIs | `ls_types::Uri` (fluent_uri, `.parse()`) |
| `LspService::new(\|client\| Backend::new(client))` | `LspService::new(Backend::new)` |
| `tower_lsp::Client` | `tower_lsp_server::Client` |
| `tower_lsp::jsonrpc::Result` | `tower_lsp_server::jsonrpc::Result` |

---

## Related

- Parsing for LSP: `tree-sitter.md`
- Graph operations: `petgraph.md`
- Error types: `error-handling.md`
- LSP Specification: https://microsoft.github.io/language-server-protocol/
- tower-lsp-server: https://github.com/tower-lsp-community/tower-lsp-server
- ls-types crate: re-exported by tower-lsp-server, not imported directly
