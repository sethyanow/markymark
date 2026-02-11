//! markymark-mcp: MCP server implementation using rmcp
//!
//! Provides Model Context Protocol support for markdown tools.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::sync::Arc;

use anyhow::Context as _;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

mod runtime_engine;

pub use runtime_engine::RuntimeEngine;

/// Request payload for `get-outline`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutlineRequest {
    /// Document URI (`file://...`) to inspect.
    pub uri: String,
}

/// Response payload for `get-outline`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OutlineResponse {
    /// Input document URI.
    pub uri: String,
    /// Heading outline entries.
    pub headings: Vec<String>,
}

/// Request payload for `search-symbols`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchSymbolsRequest {
    /// Query text to match against symbols.
    pub query: String,
}

/// Position payload in MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositionDto {
    /// 0-based line.
    pub line: u32,
    /// 0-based character offset.
    pub character: u32,
}

/// Range payload in MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct RangeDto {
    /// Inclusive start.
    pub start: PositionDto,
    /// Exclusive end.
    pub end: PositionDto,
}

/// Symbol match payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolMatchDto {
    /// Symbol text.
    pub name: String,
    /// Document URI where symbol appears.
    pub uri: String,
    /// Symbol location.
    pub range: RangeDto,
}

/// Response payload for `search-symbols`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SearchSymbolsResponse {
    /// Query text used for search.
    pub query: String,
    /// Deterministically ordered matches.
    pub symbols: Vec<SymbolMatchDto>,
}

/// Tool error envelope for consistent structured failures.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ToolErrorEnvelope {
    /// Error body.
    pub error: ToolErrorPayload,
}

/// Tool error payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ToolErrorPayload {
    /// Stable machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MarkymarkMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "markymark MCP tools for markdown outline and symbol search".to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }
}

/// Minimal MCP-side facade that forwards transport requests to the shared core engine.
///
/// This keeps transport-specific crates thin and makes LSP/MCP behavior converge on
/// the same operation model.
pub struct MarkymarkMcp {
    engine: Arc<dyn CoreEngine>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl MarkymarkMcp {
    /// Construct an MCP facade from a shared core engine implementation.
    pub fn new(engine: Arc<dyn CoreEngine>) -> Self {
        Self {
            engine,
            tool_router: Self::tool_router(),
        }
    }

    /// Request a document outline from the core engine.
    pub fn get_outline(&self, uri: DocumentUri) -> CoreOperationResult {
        self.engine.execute(CoreOperation::GetOutline { uri })
    }

    /// Request symbol search from the core engine.
    pub fn search_symbols(&self, query: String) -> CoreOperationResult {
        self.engine.execute(CoreOperation::SearchSymbols { query })
    }

    /// Request references at a target range.
    pub fn find_references(&self, uri: DocumentUri, position: Range) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::FindReferences { uri, position })
    }

    /// Request rename operation at a target range.
    pub fn rename(
        &self,
        uri: DocumentUri,
        position: Range,
        new_name: String,
    ) -> CoreOperationResult {
        self.engine.execute(CoreOperation::Rename {
            uri,
            position,
            new_name,
        })
    }

    /// Start an MCP server on stdio transport and block until shutdown.
    pub async fn serve_stdio(self) -> anyhow::Result<()> {
        let server = self
            .serve(rmcp::transport::io::stdio())
            .await
            .context("failed to initialize MCP stdio transport")?;
        let _quit_reason = server
            .waiting()
            .await
            .context("markymark MCP stdio server exited with error")?;
        Ok(())
    }

    /// Get heading outline entries from a markdown document.
    #[tool(
        name = "get-outline",
        description = "Get heading outline for a markdown file URI."
    )]
    pub async fn get_outline_tool(
        &self,
        params: Parameters<OutlineRequest>,
    ) -> Result<CallToolResult, McpError> {
        let uri = match parse_file_uri(&params.0.uri) {
            Ok(uri) => uri,
            Err(err) => return Ok(tool_error(&err.code, err.message)),
        };

        match self.get_outline(uri) {
            CoreOperationResult::Outline(headings) => {
                Ok(CallToolResult::structured(json!(OutlineResponse {
                    uri: params.0.uri,
                    headings,
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("get-outline", &other)),
        }
    }

    /// Search symbols across indexed markdown documents.
    #[tool(
        name = "search-symbols",
        description = "Search symbols by query string across indexed documents."
    )]
    pub async fn search_symbols_tool(
        &self,
        params: Parameters<SearchSymbolsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let query = params.0.query.trim().to_string();
        if query.is_empty() {
            return Ok(tool_error(
                "invalid_query",
                "query must not be empty for search-symbols",
            ));
        }

        match self.search_symbols(query.clone()) {
            CoreOperationResult::Symbols(symbols) => {
                let mut mapped: Vec<SymbolMatchDto> = symbols
                    .into_iter()
                    .map(|(name, uri, range)| SymbolMatchDto {
                        name,
                        uri: uri.as_str().to_string(),
                        range: range_to_dto(range),
                    })
                    .collect();
                // Keep output ordering deterministic for stable clients/tests.
                mapped.sort();

                Ok(CallToolResult::structured(json!(SearchSymbolsResponse {
                    query,
                    symbols: mapped,
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("search-symbols", &other)),
        }
    }
}

/// Run markymark MCP over stdio using the provided shared core engine.
pub async fn run_stdio(engine: Arc<dyn CoreEngine>) -> anyhow::Result<()> {
    MarkymarkMcp::new(engine).serve_stdio().await
}

fn parse_file_uri(uri: &str) -> Result<DocumentUri, ToolErrorPayload> {
    if !uri.starts_with("file://") {
        return Err(ToolErrorPayload {
            code: "non_file_uri".to_string(),
            message: format!("only file:// URIs are supported, got: {uri}"),
        });
    }
    DocumentUri::new(uri).map_err(|err| ToolErrorPayload {
        code: "invalid_uri".to_string(),
        message: err.to_string(),
    })
}

fn tool_error(code: &str, message: impl Into<String>) -> CallToolResult {
    CallToolResult::structured_error(json!(ToolErrorEnvelope {
        error: ToolErrorPayload {
            code: code.to_string(),
            message: message.into(),
        }
    }))
}

fn tool_error_from_core(err: CoreError) -> CallToolResult {
    match err {
        CoreError::InvalidUri(message) => tool_error("invalid_uri", message),
        CoreError::NotImplemented(message) => tool_error("not_implemented", message),
        CoreError::Message(message) => tool_error("core_error", message),
    }
}

fn unexpected_result_error(tool: &str, result: &CoreOperationResult) -> CallToolResult {
    tool_error(
        "unexpected_core_result",
        format!("tool {tool} received unsupported core result variant: {result:?}"),
    )
}

fn range_to_dto(range: Range) -> RangeDto {
    RangeDto {
        start: position_to_dto(range.start),
        end: position_to_dto(range.end),
    }
}

fn position_to_dto(position: Position) -> PositionDto {
    PositionDto {
        line: position.line,
        character: position.character,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    struct MockEngine {
        mode: MockMode,
    }

    enum MockMode {
        Happy,
        CoreError,
        UnsortedSymbols,
    }

    impl CoreEngine for MockEngine {
        fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
            match (&self.mode, operation) {
                (MockMode::CoreError, _) => {
                    CoreOperationResult::Error(CoreError::Message("engine failed".to_string()))
                }
                (_, CoreOperation::GetOutline { .. }) => {
                    CoreOperationResult::Outline(vec!["Heading".to_string()])
                }
                (MockMode::UnsortedSymbols, CoreOperation::SearchSymbols { .. }) => {
                    CoreOperationResult::Symbols(vec![
                        (
                            "zeta".to_string(),
                            DocumentUri::from_file_path(Path::new("/vault/b.md")),
                            Range::new(Position::new(10, 1), Position::new(10, 5)),
                        ),
                        (
                            "alpha".to_string(),
                            DocumentUri::from_file_path(Path::new("/vault/a.md")),
                            Range::new(Position::new(1, 0), Position::new(1, 4)),
                        ),
                    ])
                }
                (_, CoreOperation::SearchSymbols { query }) => {
                    CoreOperationResult::Symbols(vec![(
                        query,
                        DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                        Range::new(Position::new(0, 0), Position::new(0, 7)),
                    )])
                }
                (_, CoreOperation::FindReferences { .. }) => {
                    CoreOperationResult::Locations(vec![(
                        DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                        Range::new(Position::new(1, 0), Position::new(1, 5)),
                    )])
                }
                (_, CoreOperation::Rename { new_name, .. }) => {
                    CoreOperationResult::WorkspaceEdit(vec![(
                        DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                        vec![(
                            Range::new(Position::new(2, 0), Position::new(2, 7)),
                            new_name,
                        )],
                    )])
                }
            }
        }
    }

    #[test]
    fn forwards_get_outline_to_core_engine() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let uri = DocumentUri::from_file_path(Path::new("/vault/notes.md"));
        let result = mcp.get_outline(uri);

        match result {
            CoreOperationResult::Outline(items) => {
                assert_eq!(items, vec!["Heading".to_string()]);
            }
            _ => panic!("expected outline result"),
        }
    }

    #[test]
    fn forwards_search_symbols_to_core_engine() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let result = mcp.search_symbols("intro".to_string());

        match result {
            CoreOperationResult::Symbols(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].0, "intro");
            }
            _ => panic!("expected symbols result"),
        }
    }

    #[test]
    fn registers_expected_rmcp_tools() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let tools = mcp.tool_router.list_all();
        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"get-outline"));
        assert!(names.contains(&"search-symbols"));
    }

    #[tokio::test]
    async fn outline_tool_returns_structured_success() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let result = mcp
            .get_outline_tool(Parameters(OutlineRequest {
                uri: "file:///vault/notes.md".to_string(),
            }))
            .await
            .expect("tool call should not return protocol error");

        assert_eq!(result.is_error, Some(false));
        let payload: OutlineResponse = result.into_typed().expect("typed outline response");
        assert_eq!(payload.uri, "file:///vault/notes.md");
        assert_eq!(payload.headings, vec!["Heading".to_string()]);
    }

    #[tokio::test]
    async fn outline_tool_rejects_non_file_uri() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let result = mcp
            .get_outline_tool(Parameters(OutlineRequest {
                uri: "https://example.com/notes.md".to_string(),
            }))
            .await
            .expect("tool call should not return protocol error");

        assert_eq!(result.is_error, Some(true));
        let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
        assert_eq!(payload.error.code, "non_file_uri");
    }

    #[tokio::test]
    async fn search_symbols_tool_rejects_empty_query() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::Happy,
        }));
        let result = mcp
            .search_symbols_tool(Parameters(SearchSymbolsRequest {
                query: "   ".to_string(),
            }))
            .await
            .expect("tool call should not return protocol error");

        assert_eq!(result.is_error, Some(true));
        let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
        assert_eq!(payload.error.code, "invalid_query");
    }

    #[tokio::test]
    async fn search_symbols_tool_orders_results_deterministically() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::UnsortedSymbols,
        }));
        let result = mcp
            .search_symbols_tool(Parameters(SearchSymbolsRequest {
                query: "anything".to_string(),
            }))
            .await
            .expect("tool call should not return protocol error");

        assert_eq!(result.is_error, Some(false));
        let payload: SearchSymbolsResponse = result.into_typed().expect("typed response");
        assert_eq!(payload.symbols.len(), 2);
        assert_eq!(payload.symbols[0].name, "alpha");
        assert_eq!(payload.symbols[1].name, "zeta");
    }

    #[tokio::test]
    async fn tool_errors_map_core_failures_consistently() {
        let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
            mode: MockMode::CoreError,
        }));
        let result = mcp
            .search_symbols_tool(Parameters(SearchSymbolsRequest {
                query: "intro".to_string(),
            }))
            .await
            .expect("tool call should not return protocol error");

        assert_eq!(result.is_error, Some(true));
        let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
        assert_eq!(payload.error.code, "core_error");
    }
}
