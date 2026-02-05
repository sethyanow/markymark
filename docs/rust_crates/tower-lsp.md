# tower-lsp - LSP Server Framework

<agent>
<goal>Build async LSP servers with proper state management and capability negotiation.</goal>
<when_to_use>When implementing Language Server Protocol servers in Rust.</when_to_use>
<contains>Server setup, LanguageServer trait, state patterns, capability config, notifications, custom methods</contains>
<see_also>tree-sitter.md, petgraph.md, error-handling.md</see_also>
</agent>

**TL;DR:** tower-lsp provides async LSP server infrastructure. Implement `LanguageServer` trait, use `Client` for notifications, manage state with interior mutability.

**Checklist:**
- [ ] Implement `LanguageServer` trait with async methods
- [ ] Use `Client` for sending notifications/requests to editor
- [ ] Declare capabilities in `initialize` response
- [ ] Handle state with `tokio::sync::RwLock` or similar
- [ ] Use `jsonrpc::Result` for error responses

---

## Setup

### Cargo.toml

```toml
[dependencies]
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Basic Server Structure

```rust
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tokio::sync::RwLock;
use std::sync::Arc;

struct Backend {
    client: Client,
    state: Arc<RwLock<ServerState>>,
}

#[derive(Default)]
struct ServerState {
    documents: HashMap<Url, String>,
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

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        ..Default::default()
                    }
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
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

    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

---

## Patterns

### Document Synchronization

```rust
#[tower_lsp::async_trait]
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
            // Apply incremental changes
            for change in params.content_changes {
                if let Some(range) = change.range {
                    // Convert LSP range to byte offsets and apply
                    apply_change(doc, range, &change.text);
                } else {
                    // Full document replacement
                    *doc = change.text;
                }
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
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

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
    async fn publish_diagnostics(&self, uri: Url) {
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
let (service, socket) = LspService::build(|client| Backend::new(client))
    .custom_method("markymark/createRealm", Backend::create_realm)
    .custom_method("markymark/destroyRealm", Backend::destroy_realm)
    .finish();
```

---

## Pitfalls

### Lock Ordering - Deadlock Risk

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
</pitfall>

### TextDocumentSyncKind Mismatch

<pitfall>
**Problem:** Declaring `INCREMENTAL` sync but handling as `FULL` causes corruption.

**Solution:** Match your `did_change` implementation to declared capability:

```rust
// If you declare INCREMENTAL:
TextDocumentSyncKind::INCREMENTAL

// You MUST apply changes incrementally:
for change in params.content_changes {
    if let Some(range) = change.range {
        // Apply at specific range
        apply_incremental_change(doc, range, &change.text);
    } else {
        // Fallback to full replacement
        *doc = change.text;
    }
}
```
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

async fn publish_diagnostics(&self, uri: Url) {
    if !self.initialized.load(Ordering::SeqCst) {
        return; // Client not ready
    }
    // ...
}
```
</pitfall>

---

## Related

- Parsing for LSP: `tree-sitter.md`
- Graph operations: `petgraph.md`
- Error types: `error-handling.md`
- LSP Specification: https://microsoft.github.io/language-server-protocol/
- tower-lsp examples: https://github.com/ebkalderon/tower-lsp/tree/master/examples
