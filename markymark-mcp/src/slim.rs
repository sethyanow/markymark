//! Slim MCP router: a single `execute` tool that routes to all existing
//! MCP operations by operation name.
//!
//! This dramatically reduces the tool schema footprint (~500 tokens vs ~10k)
//! while preserving full functionality. Designed for Claude Code agents where
//! skills provide the workflow guidance and the router provides the raw operations.

use std::sync::Arc;

use anyhow::Context as _;
use markymark_core::engine::CoreEngine;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::subscriptions;
use crate::tools;

/// Request payload for the single `execute` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteRequest {
    /// The operation to execute. Available operations: get-outline, search-symbols,
    /// semantic-search, find-references, rename, create-realm, destroy-realm,
    /// add-root, remove-root, realm-stats, export-index, search-workspace,
    /// search-for-pattern, graph-analysis, get-diagnostics, export-docs-index,
    /// enrich-document, recommend-docs, curation-diagnostics, get-content-blocks,
    /// search-block-text.
    pub operation: String,
    /// Operation-specific parameters as a JSON object. Each operation accepts
    /// the same parameters as its full MCP tool equivalent.
    pub params: serde_json::Value,
}

/// Slim MCP server facade exposing a single `execute` tool that routes to all
/// existing MCP operations.
pub struct SlimMarkymarkMcp {
    engine: Arc<dyn CoreEngine>,
    tool_router: ToolRouter<Self>,
    subscriptions: subscriptions::SubscriptionTracker,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SlimMarkymarkMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Slim markymark MCP router. Use execute({operation, params}) for all operations."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }
}

#[tool_router(router = tool_router)]
impl SlimMarkymarkMcp {
    /// Construct a slim MCP facade from a shared core engine.
    pub fn new(engine: Arc<dyn CoreEngine>) -> Self {
        let tool_router = Self::tool_router();
        Self {
            engine,
            tool_router,
            subscriptions: subscriptions::SubscriptionTracker::new(),
        }
    }

    /// List all registered tool definitions (for testing/introspection).
    pub fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

    /// Start a slim MCP server on stdio transport and block until shutdown.
    pub async fn serve_stdio(self) -> anyhow::Result<()> {
        let server = self
            .serve(rmcp::transport::io::stdio())
            .await
            .context("failed to initialize slim MCP stdio transport")?;
        server
            .waiting()
            .await
            .context("markymark slim MCP server exited with error")?;
        Ok(())
    }

    // ---- Single router tool ----

    /// Execute a markymark operation by name. Routes to the same handlers as the
    /// full MCP tool surface but through a single unified entry point.
    #[tool(
        name = "execute",
        description = "Execute a markymark operation. Operations: get-outline, search-symbols, semantic-search, find-references, rename, create-realm, destroy-realm, add-root, remove-root, realm-stats, export-index, search-workspace, search-for-pattern, graph-analysis, get-diagnostics, export-docs-index, enrich-document, recommend-docs, curation-diagnostics, get-content-blocks, search-block-text."
    )]
    pub async fn execute_tool(
        &self,
        request: Parameters<ExecuteRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = request.0;
        self.dispatch(&req.operation, req.params).await
    }
}

impl SlimMarkymarkMcp {
    async fn dispatch(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        macro_rules! route {
            ($dto:ty, $handler:expr, $params:expr) => {
                match deser::<$dto>($params) {
                    Ok(req) => $handler(&*self.engine, req).await,
                    Err(msg) => Ok(tools::tool_error("invalid_params", msg)),
                }
            };
        }

        macro_rules! route_realm {
            ($dto:ty, $handler:expr, $params:expr) => {
                match deser::<$dto>($params) {
                    Ok(req) => {
                        let r = $handler(&*self.engine, req).await;
                        if r.notify {
                            self.subscriptions.notify_all().await;
                        }
                        r.result
                    }
                    Err(msg) => Ok(tools::tool_error("invalid_params", msg)),
                }
            };
        }

        match operation {
            "get-outline" => route!(
                crate::dto::OutlineRequest,
                tools::outline::handle_get_outline,
                params
            ),
            "search-symbols" => route!(
                crate::dto::SearchSymbolsRequest,
                tools::search::handle_search_symbols,
                params
            ),
            "semantic-search" => {
                #[cfg(feature = "semantic-search")]
                {
                    route!(
                        crate::dto::SemanticSearchRequest,
                        tools::search::handle_semantic_search,
                        params
                    )
                }
                #[cfg(not(feature = "semantic-search"))]
                {
                    let _ = params;
                    Ok(tools::tool_error(
                        "unsupported",
                        "semantic-search requires the semantic-search feature",
                    ))
                }
            }
            "find-references" => route!(
                crate::dto::FindReferencesRequest,
                tools::refs::handle_find_references,
                params
            ),
            "rename" => route!(
                crate::dto::RenameRequest,
                tools::refs::handle_rename,
                params
            ),
            "create-realm" => route_realm!(
                crate::dto::CreateRealmRequest,
                tools::realm::handle_create_realm,
                params
            ),
            "destroy-realm" => route_realm!(
                crate::dto::DestroyRealmRequest,
                tools::realm::handle_destroy_realm,
                params
            ),
            "add-root" => route_realm!(
                crate::dto::AddRootRequest,
                tools::realm::handle_add_root,
                params
            ),
            "remove-root" => route_realm!(
                crate::dto::RemoveRootRequest,
                tools::realm::handle_remove_root,
                params
            ),
            "realm-stats" => route!(
                crate::dto::RealmStatsRequest,
                tools::realm::handle_realm_stats,
                params
            ),
            "export-index" => route!(
                crate::dto::ExportIndexRequest,
                tools::outline::handle_export_index,
                params
            ),
            "search-workspace" => route!(
                crate::dto::SearchWorkspaceRequest,
                tools::search::handle_search_workspace,
                params
            ),
            "search-for-pattern" => route!(
                crate::dto::SearchForPatternRequest,
                tools::search::handle_search_for_pattern,
                params
            ),
            "graph-analysis" => route!(
                crate::dto::GraphAnalysisRequest,
                tools::graph::handle_graph_analysis,
                params
            ),
            "get-diagnostics" => route!(
                crate::dto::GetDiagnosticsRequest,
                tools::diagnostics::handle_get_diagnostics,
                params
            ),
            "export-docs-index" => route!(
                crate::dto::ExportDocsIndexRequest,
                tools::export_docs_index::handle_export_docs_index,
                params
            ),
            "enrich-document" => route!(
                crate::dto::EnrichDocumentRequest,
                tools::enrich::handle_enrich_document,
                params
            ),
            "recommend-docs" => route!(
                crate::dto::RecommendDocsRequest,
                tools::recommend::handle_recommend_docs,
                params
            ),
            "curation-diagnostics" => route!(
                crate::dto::CurationDiagnosticsRequest,
                tools::curation::handle_curation_diagnostics,
                params
            ),
            "get-content-blocks" => route!(
                crate::dto::GetContentBlocksRequest,
                tools::blocks::handle_get_content_blocks,
                params
            ),
            "search-block-text" => route!(
                crate::dto::SearchBlockTextRequest,
                tools::blocks::handle_search_block_text,
                params
            ),
            unknown => Ok(tools::tool_error(
                "unknown_operation",
                format!(
                    "Unknown operation: '{unknown}'. Available: get-outline, search-symbols, \
                     semantic-search, find-references, rename, create-realm, destroy-realm, \
                     add-root, remove-root, realm-stats, export-index, search-workspace, \
                     search-for-pattern, graph-analysis, get-diagnostics, export-docs-index, \
                     enrich-document, recommend-docs, curation-diagnostics, get-content-blocks, \
                     search-block-text"
                ),
            )),
        }
    }
}

/// Deserialize a JSON value into the target DTO.
fn deser<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid parameters: {e}"))
}

/// Run the slim MCP router over stdio.
pub async fn run_slim_stdio(engine: Arc<dyn CoreEngine>) -> anyhow::Result<()> {
    SlimMarkymarkMcp::new(engine).serve_stdio().await
}
