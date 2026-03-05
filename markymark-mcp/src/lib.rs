//! markymark-mcp: MCP server implementation using rmcp
//!
//! Provides Model Context Protocol support for markdown tools.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::sync::Arc;

use anyhow::Context as _;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Range};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, GetPromptRequestParams, GetPromptResult, ListPromptsResult,
        ListResourceTemplatesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, ServerCapabilities, ServerInfo, SubscribeRequestParams,
        UnsubscribeRequestParams,
    },
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};

pub mod dto;
mod engine;
mod graph;
mod pattern;
mod prompts;
mod rename_ops;
mod resources;
pub(crate) mod search;
mod subscriptions;
mod tools;

pub use dto::*;
#[cfg(feature = "semantic-search")]
pub use engine::HashEmbeddingProvider;
pub use engine::RuntimeEngine;

pub(crate) const SEMANTIC_SEARCH_MAX_TOP_K: u32 = 100;

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MarkymarkMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "markymark MCP tools and resources for markdown indexing".to_string(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .build(),
            ..ServerInfo::default()
        }
    }

    fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        self.subscriptions.subscribe(request.uri, context.peer);
        std::future::ready(Ok(()))
    }

    fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        self.subscriptions.untrack(&request.uri);
        std::future::ready(Ok(()))
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListPromptsResult {
            prompts: self.list_prompt_definitions(),
            next_cursor: None,
            meta: None,
        }))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        self.get_prompt_by_name(&request.name, request.arguments)
            .await
    }

    fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_
    {
        std::future::ready(Ok(ListResourceTemplatesResult {
            resource_templates: self.resource_templates(),
            next_cursor: None,
            meta: None,
        }))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let result = self.read_resource(&request.uri).await;
        result.map(|contents| ReadResourceResult { contents })
    }
}

/// Minimal MCP-side facade that forwards transport requests to the shared core engine.
///
/// This keeps transport-specific crates thin and makes LSP/MCP behavior converge on
/// the same operation model.
pub struct MarkymarkMcp {
    engine: Arc<dyn CoreEngine>,
    tool_router: ToolRouter<Self>,
    subscriptions: subscriptions::SubscriptionTracker,
}

#[tool_router(router = tool_router)]
impl MarkymarkMcp {
    /// Construct an MCP facade from a shared core engine implementation.
    pub fn new(engine: Arc<dyn CoreEngine>) -> Self {
        #[cfg(feature = "semantic-search")]
        let tool_router = Self::tool_router();

        #[cfg(not(feature = "semantic-search"))]
        let tool_router = {
            let mut router = Self::tool_router();
            router.remove_route("semantic-search");
            router
        };

        Self {
            engine,
            tool_router,
            subscriptions: subscriptions::SubscriptionTracker::new(),
        }
    }

    /// Record a resource URI as subscribed (without peer handle, for testing).
    pub fn track_subscription(&self, uri: String) {
        self.subscriptions.track(uri);
    }

    /// Remove a resource URI subscription. Returns `true` if it was subscribed.
    pub fn untrack_subscription(&self, uri: &str) -> bool {
        self.subscriptions.untrack(uri)
    }

    /// Check if a resource URI is currently subscribed.
    pub fn is_subscribed(&self, uri: &str) -> bool {
        self.subscriptions.is_subscribed(uri)
    }

    /// Return the count of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.subscription_count()
    }

    /// Request a document outline from the core engine.
    pub async fn get_outline(
        &self,
        uri: DocumentUri,
        realm: Option<String>,
    ) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::GetOutline {
                uri,
                realm,
                format: "flat".to_string(),
                include_text: false,
            })
            .await
    }

    /// Request symbol search from the core engine.
    pub async fn search_symbols(
        &self,
        query: String,
        realm: Option<String>,
    ) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::SearchSymbols { query, realm })
            .await
    }

    /// Request semantic search from the core engine.
    pub async fn semantic_search(
        &self,
        query: String,
        realm: Option<String>,
        top_k: u32,
        min_score: f32,
    ) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::SemanticSearch {
                query,
                realm,
                top_k,
                min_score,
            })
            .await
    }

    /// Request references at a target range.
    pub async fn find_references(
        &self,
        uri: DocumentUri,
        position: Range,
        realm: Option<String>,
    ) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::FindReferences {
                uri,
                position,
                realm,
            })
            .await
    }

    /// Request rename operation at a target range.
    pub async fn rename(
        &self,
        uri: DocumentUri,
        position: Range,
        new_name: String,
        realm: Option<String>,
    ) -> CoreOperationResult {
        self.engine
            .execute(CoreOperation::Rename {
                uri,
                position,
                new_name,
                realm,
            })
            .await
    }

    /// List all registered tool definitions (for testing/introspection).
    pub fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
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

    // ---- Tool handlers (bodies in tools/ submodule) ----

    /// Get heading outline entries from a markdown document.
    #[tool(
        name = "get-outline",
        description = "Get heading outline for a markdown file URI."
    )]
    pub async fn get_outline_tool(
        &self,
        params: Parameters<OutlineRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::outline::handle_get_outline(&*self.engine, params.0).await
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
        tools::search::handle_search_symbols(&*self.engine, params.0).await
    }

    /// Search semantically similar sections across indexed documents.
    #[tool(
        name = "semantic-search",
        description = "Search semantically similar sections by embedding query text."
    )]
    pub async fn semantic_search_tool(
        &self,
        params: Parameters<SemanticSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::search::handle_semantic_search(&*self.engine, params.0).await
    }

    /// Find all references to a symbol at the given position.
    #[tool(
        name = "find-references",
        description = "Find all references to a heading or XML tag at a position."
    )]
    pub async fn find_references_tool(
        &self,
        params: Parameters<FindReferencesRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::refs::handle_find_references(&*self.engine, params.0).await
    }

    /// Rename a heading or XML tag and all its references.
    #[tool(
        name = "rename",
        description = "Rename a heading or XML tag at a position, updating all references."
    )]
    pub async fn rename_tool(
        &self,
        params: Parameters<RenameRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::refs::handle_rename(&*self.engine, params.0).await
    }

    /// Create a new named realm.
    #[tool(
        name = "create-realm",
        description = "Create a new named realm for isolated markdown workspace indexing."
    )]
    pub async fn create_realm_tool(
        &self,
        params: Parameters<CreateRealmRequest>,
    ) -> Result<CallToolResult, McpError> {
        let r = tools::realm::handle_create_realm(&*self.engine, params.0).await;
        if r.notify {
            self.subscriptions.notify_all().await;
        }
        r.result
    }

    /// Destroy a named realm and all its indexed documents.
    #[tool(
        name = "destroy-realm",
        description = "Destroy a named realm and unindex all its documents."
    )]
    pub async fn destroy_realm_tool(
        &self,
        params: Parameters<DestroyRealmRequest>,
    ) -> Result<CallToolResult, McpError> {
        let r = tools::realm::handle_destroy_realm(&*self.engine, params.0).await;
        if r.notify {
            self.subscriptions.notify_all().await;
        }
        r.result
    }

    /// Add a workspace root to a realm and index its markdown files.
    #[tool(
        name = "add-root",
        description = "Add a workspace root directory to a realm, indexing all markdown files."
    )]
    pub async fn add_root_tool(
        &self,
        params: Parameters<AddRootRequest>,
    ) -> Result<CallToolResult, McpError> {
        let r = tools::realm::handle_add_root(&*self.engine, params.0).await;
        if r.notify {
            self.subscriptions.notify_all().await;
        }
        r.result
    }

    /// Remove a workspace root from a realm, unindexing its documents.
    #[tool(
        name = "remove-root",
        description = "Remove a workspace root from a realm, unindexing all its documents."
    )]
    pub async fn remove_root_tool(
        &self,
        params: Parameters<RemoveRootRequest>,
    ) -> Result<CallToolResult, McpError> {
        let r = tools::realm::handle_remove_root(&*self.engine, params.0).await;
        if r.notify {
            self.subscriptions.notify_all().await;
        }
        r.result
    }

    /// Get aggregate statistics for a realm.
    #[tool(
        name = "realm-stats",
        description = "Get aggregate statistics (document, heading, tag, link counts) for a realm."
    )]
    pub async fn realm_stats_tool(
        &self,
        params: Parameters<RealmStatsRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::realm::handle_realm_stats(&*self.engine, params.0).await
    }

    /// Export the full document index for a single document.
    #[tool(
        name = "export-index",
        description = "Export headings, XML tags, wiki links, and markdown links for a document."
    )]
    pub async fn export_index_tool(
        &self,
        params: Parameters<ExportIndexRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::outline::handle_export_index(&*self.engine, params.0).await
    }

    /// Search workspace documents by text, frontmatter, properties, or tags.
    #[tool(
        name = "search-workspace",
        description = "Search workspace documents by free text, frontmatter, Logseq properties, or tags. Returns ranked results with metadata preview."
    )]
    pub async fn search_workspace_tool(
        &self,
        params: Parameters<SearchWorkspaceRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::search::handle_search_workspace(&*self.engine, params.0).await
    }

    /// Search workspace files by regex pattern with optional glob file filtering.
    #[tool(
        name = "search-for-pattern",
        description = "Search workspace files by regex pattern. Supports glob file filtering (e.g. '*.md', '**/*.rs'), case-insensitive matching, and context lines. Returns 0-based line/column numbers consistent with LSP conventions."
    )]
    pub async fn search_for_pattern_tool(
        &self,
        params: Parameters<SearchForPatternRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::search::handle_search_for_pattern(&*self.engine, params.0).await
    }

    /// Analyse the link graph: orphans, hubs, broken links, clusters, and summary stats.
    #[tool(
        name = "graph-analysis",
        description = "Analyse the markdown link graph of a workspace realm. Returns orphan documents (no resolved links in or out), hub documents (most incoming links), broken links (unresolvable wiki or markdown links), and summary statistics. Optionally computes weakly-connected clusters."
    )]
    pub async fn graph_analysis_tool(
        &self,
        params: Parameters<GraphAnalysisRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::graph::handle_graph_analysis(&*self.engine, params.0).await
    }

    /// Compute diagnostics (broken links, duplicate headings, unclosed XML tags) for a file or
    /// all files in a realm.
    #[tool(
        name = "get-diagnostics",
        description = "Get diagnostics (broken links, duplicate headings, unclosed XML tags) for a specific file or all files in a realm. Returns per-file diagnostic lists with location, severity, and message."
    )]
    pub async fn get_diagnostics_tool(
        &self,
        params: Parameters<GetDiagnosticsRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::diagnostics::handle_get_diagnostics(&*self.engine, params.0).await
    }

    /// Export a pipe-delimited docs_index block from realm state, matching the format
    /// used in CLAUDE.md for ambient agent documentation awareness.
    #[tool(
        name = "export-docs-index",
        description = "Generate a pipe-delimited docs_index block from a realm's indexed documents. Output is ready to paste into CLAUDE.md for ambient agent doc awareness. Groups files by directory (category) with deterministic sorting."
    )]
    pub async fn export_docs_index_tool(
        &self,
        params: Parameters<ExportDocsIndexRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::export_docs_index::handle_export_docs_index(&*self.engine, params.0).await
    }

    /// Enrich a document's outline with LLM-generated summaries stored in sidecar files.
    #[tool(
        name = "enrich-document",
        description = "Enrich a document's outline with LLM-generated summaries. Summaries are stored in sidecar JSON files under .markymark/ (or a custom directory). Requires an inference provider to be configured. Skips enrichment if the sidecar is fresh (content hash matches) unless force=true."
    )]
    pub async fn enrich_document_tool(
        &self,
        params: Parameters<EnrichDocumentRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::enrich::handle_enrich_document(&*self.engine, params.0).await
    }

    /// Recommend documents matching an intent query using combined text search and graph analysis.
    #[tool(
        name = "recommend-docs",
        description = "Recommend documents matching an intent query. Combines text search relevance with graph hub scores for two-stage retrieval. Returns ranked documents with optional section summaries from enrichment sidecars. Use this tool when an agent needs to find the most relevant documentation for a given task or question."
    )]
    pub async fn recommend_docs_tool(
        &self,
        params: Parameters<RecommendDocsRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::recommend::handle_recommend_docs(&*self.engine, params.0).await
    }

    /// Run curation diagnostics on a realm to detect orphans, low-connectivity docs, and suggest cross-links.
    #[tool(
        name = "curation-diagnostics",
        description = "Analyse documentation quality for a realm. Detects orphan documents (no links in or out), low-connectivity documents, and generates actionable cross-link suggestions. Returns structured curation report with aggregate statistics including orphan percentage and average connectivity. Use this tool to identify documentation quality gaps and get specific improvement recommendations."
    )]
    pub async fn curation_diagnostics_tool(
        &self,
        params: Parameters<CurationDiagnosticsRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::curation::handle_curation_diagnostics(&*self.engine, params.0).await
    }
}

/// Run markymark MCP over stdio using the provided shared core engine.
pub async fn run_stdio(engine: Arc<dyn CoreEngine>) -> anyhow::Result<()> {
    MarkymarkMcp::new(engine).serve_stdio().await
}
