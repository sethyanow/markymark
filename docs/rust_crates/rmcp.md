# rmcp - Official Rust MCP SDK

<agent>
<goal>Build MCP servers exposing tools, resources, and prompts to AI assistants.</goal>
<when_to_use>When implementing Model Context Protocol servers in Rust.</when_to_use>
<contains>ServerHandler trait, #[tool] macro, transport setup, resources, prompts, structured output</contains>
<see_also>tower-lsp.md, error-handling.md, core.md</see_also>
</agent>

**TL;DR:** rmcp is Anthropic's official Rust MCP SDK. Implement `ServerHandler`, define tools with `#[tool]` macro, serve over stdio or HTTP streaming.

**Checklist:**
- [ ] Add `rmcp` with `server` feature to Cargo.toml
- [ ] Implement `ServerHandler` trait with `get_info()` returning capabilities
- [ ] Use `#[tool_router]` and `#[tool_handler]` macros on your handler struct
- [ ] Define tools with `#[tool(description = "...")]` on async methods
- [ ] Use `Parameters<T>` for typed input and return `CallToolResult`
- [ ] Set up transport (stdio or HTTP streaming)
- [ ] Call `.serve(transport).await?` to start server

---

## Setup

### Cargo.toml

```toml
[dependencies]
rmcp = { version = "0.13", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "0.8"
anyhow = "1"
```

### Feature Flags

| Feature | Purpose |
|---------|---------|
| `server` | Server + tool system (required for MCP servers) |
| `client` | Client functionality (for connecting to other MCP servers) |
| `macros` | `#[tool]` macro support (included by default) |
| `transport-io` | Stdio transport (typical for CLI tools) |
| `transport-child-process` | Spawn MCP servers as subprocesses |
| `transport-streamable-http-server` | HTTP streaming server transport |
| `transport-streamable-http-client` | HTTP streaming client transport |
| `auth` | OAuth2 authentication support |

---

## Patterns

### Basic MCP Server

```rust
use rmcp::{ServerHandler, ServiceExt};
use rmcp::model::*;
use rmcp::handler::server::tool::ToolCallContext;

struct MarkymarkMcp {
    core: Arc<dyn CoreEngine>,
}

#[tool_router]
impl MarkymarkMcp {
    #[tool(description = "Find all references to a symbol")]
    async fn find_references(
        &self,
        #[tool(param)]
        uri: String,
        #[tool(param)]
        symbol_type: String,
        #[tool(param)]
        symbol_id: String,
        #[tool(param, default = true)]
        include_declaration: bool,
    ) -> Result<CallToolResult, McpError> {
        let op = CoreOperation::FindReferences {
            symbol: parse_symbol(&uri, &symbol_type, &symbol_id)?,
            include_declaration,
        };
        let result = self.core.execute(op).await;
        match result {
            CoreResult::Locations(locs) => {
                let json = serde_json::to_string_pretty(&locs)?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            CoreResult::Error(e) => Err(McpError::internal_error(e.to_string(), None)),
            _ => unreachable!(),
        }
    }

    #[tool(description = "Rename a symbol and update all references")]
    async fn rename(
        &self,
        #[tool(param)]
        uri: String,
        #[tool(param)]
        symbol_type: String,
        #[tool(param)]
        symbol_id: String,
        #[tool(param)]
        new_name: String,
    ) -> Result<CallToolResult, McpError> {
        let op = CoreOperation::Rename {
            symbol: parse_symbol(&uri, &symbol_type, &symbol_id)?,
            new_name,
        };
        let result = self.core.execute(op).await;
        // Convert CoreResult to CallToolResult...
        todo!()
    }

    #[tool(description = "Create a new realm for workspace isolation")]
    async fn create_realm(
        &self,
        #[tool(param)]
        mode: String,
        #[tool(param)]
        roots: Vec<String>,
    ) -> Result<CallToolResult, McpError> {
        // ...
        todo!()
    }
}

#[tool_handler]
impl ServerHandler for MarkymarkMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("markymark: High-performance markdown indexing with realm isolation".into()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }
}
```

### Starting the Server

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = MarkymarkMcp::new(core_engine);

    // Stdio transport (typical for CLI)
    let transport = rmcp::transport::io::stdio();
    let server = service.serve(transport).await?;
    server.waiting().await?;

    Ok(())
}
```

### Structured Output with Json

```rust
use rmcp::handler::server::tool::{Json, Parameters};

#[derive(serde::Serialize, schemars::JsonSchema)]
struct SymbolInfo {
    name: String,
    kind: String,
    uri: String,
    range: Range,
}

#[tool(description = "Get symbol details")]
async fn get_symbol(
    &self,
    params: Parameters<GetSymbolRequest>,
) -> Result<Json<SymbolInfo>, String> {
    // Parameters<T> auto-generates JSON schema from T
    // Json<T> auto-serializes response
    let req = params.into_inner();
    let info = self.core.get_symbol(&req.uri, &req.symbol)?;
    Ok(Json(info))
}
```

### Accessing Peer for Notifications

```rust
impl ServerHandler for MarkymarkMcp {
    async fn on_progress(
        &self,
        notification: ProgressNotificationParam,
        context: NotificationContext<RoleServer>,
    ) {
        let peer = context.peer;
        peer.notify_logging_message(LoggingMessageNotificationParam {
            level: LoggingLevel::Info,
            logger: Some("markymark".into()),
            data: serde_json::json!({ "status": "indexing complete" }),
        })
        .await
        .ok();
    }
}
```

---

## Pitfalls

### Missing Feature Flags

<pitfall>
**Problem:** `rmcp` compiles but tools aren't registered, or transport doesn't work.

**Solution:** Enable the right features:
```toml
# Server with stdio transport
rmcp = { version = "0.13", features = ["server", "transport-io"] }

# Client connecting to child process
rmcp = { version = "0.13", features = ["client", "transport-child-process"] }
```

The `server` feature is required for `ServerHandler`, `#[tool]`, etc. Transport features are required for the specific transport you're using.
</pitfall>

### Schema Generation Requires schemars

<pitfall>
**Problem:** `#[tool]` macro fails to compile with cryptic errors about missing trait implementations.

**Solution:** Tool parameters need `schemars::JsonSchema` derive:
```rust
#[derive(serde::Deserialize, schemars::JsonSchema)]
struct MyToolInput {
    query: String,
    limit: Option<usize>,
}
```

Add `schemars = "0.8"` to your dependencies. The `#[tool]` macro generates JSON schema from your types at compile time.
</pitfall>

### Forgetting to Call .waiting()

<pitfall>
**Problem:** Server starts but exits immediately.

**Solution:** After `.serve()`, call `.waiting()` to keep the server alive:
```rust
let server = service.serve(transport).await?;
server.waiting().await?;  // Don't forget this!
```

Without `.waiting()`, the server task is spawned but `main` returns immediately.
</pitfall>

### Tool Return Types

<pitfall>
**Problem:** Confusion between `CallToolResult`, `Json<T>`, and raw `String` returns.

**Solution:** Three patterns for tool returns:
```rust
// 1. Raw text content
#[tool(description = "...")]
async fn my_tool(&self) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text("result")]))
}

// 2. Structured JSON (auto-schema)
#[tool(description = "...")]
async fn my_tool(&self, params: Parameters<Input>) -> Result<Json<Output>, String> {
    Ok(Json(output))
}

// 3. Simple string error
#[tool(description = "...")]
async fn my_tool(&self) -> Result<CallToolResult, String> {
    Err("something went wrong".into())
}
```
</pitfall>

---

## Related

- LSP transport: `tower-lsp.md`
- Error types: `error-handling.md`
- Core patterns: `core.md`
- rmcp docs: https://docs.rs/rmcp
- rmcp repository: https://github.com/modelcontextprotocol/rust-sdk
- MCP specification: https://modelcontextprotocol.io/
