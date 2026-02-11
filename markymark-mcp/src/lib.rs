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
    model::{
        CallToolResult, ListResourceTemplatesResult, RawResourceTemplate, ReadResourceRequestParam,
        ReadResourceResult, ResourceContents, ResourceTemplate, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::json;

pub mod dto;
mod rename_ops;
mod runtime_engine;

pub use dto::*;
pub use runtime_engine::RuntimeEngine;

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MarkymarkMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "markymark MCP tools and resources for markdown indexing".to_string(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..ServerInfo::default()
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_
    {
        std::future::ready(Ok(ListResourceTemplatesResult {
            resource_templates: self.resource_templates(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        let result = self.read_resource_sync(&request.uri);
        std::future::ready(result.map(|contents| ReadResourceResult { contents }))
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

    /// List all registered tool definitions (for testing/introspection).
    pub fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

    /// Return the MCP resource templates this server advertises.
    pub fn resource_templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                raw: RawResourceTemplate {
                    uri_template: "markymark://outline/{uri}".to_string(),
                    name: "document-outline".to_string(),
                    title: Some("Document Outline".to_string()),
                    description: Some(
                        "Get the heading outline for a markdown document by URI.".to_string(),
                    ),
                    mime_type: Some("application/json".to_string()),
                    icons: None,
                },
                annotations: None,
            },
            ResourceTemplate {
                raw: RawResourceTemplate {
                    uri_template: "markymark://symbols?query={query}".to_string(),
                    name: "symbol-search".to_string(),
                    title: Some("Symbol Search".to_string()),
                    description: Some(
                        "Search indexed markdown symbols by query string.".to_string(),
                    ),
                    mime_type: Some("application/json".to_string()),
                    icons: None,
                },
                annotations: None,
            },
            ResourceTemplate {
                raw: RawResourceTemplate {
                    uri_template: "markymark://dependency-graph?realm={realm}&format={format}"
                        .to_string(),
                    name: "dependency-graph".to_string(),
                    title: Some("Dependency Graph".to_string()),
                    description: Some(
                        "Inter-document link graph in JSON or DOT format.".to_string(),
                    ),
                    mime_type: Some("application/json".to_string()),
                    icons: None,
                },
                annotations: None,
            },
        ]
    }

    /// Synchronously read an MCP resource by URI.
    ///
    /// Dispatches based on the URI scheme/path prefix:
    /// - `markymark://outline/{uri}` → document outline
    /// - `markymark://symbols?query={query}` → symbol search
    /// - `markymark://dependency-graph?realm={realm}&format={format}` → link graph
    pub fn read_resource_sync(&self, uri: &str) -> Result<Vec<ResourceContents>, McpError> {
        if let Some(doc_uri) = uri.strip_prefix("markymark://outline/") {
            return self.read_outline_resource(uri, doc_uri);
        }
        if uri.starts_with("markymark://symbols") {
            return self.read_symbols_resource(uri);
        }
        if uri.starts_with("markymark://dependency-graph") {
            return self.read_dependency_graph_resource(uri);
        }
        Err(McpError::resource_not_found(
            format!("unknown resource URI: {uri}"),
            None,
        ))
    }

    fn read_outline_resource(
        &self,
        resource_uri: &str,
        doc_uri_str: &str,
    ) -> Result<Vec<ResourceContents>, McpError> {
        let doc_uri = DocumentUri::new(doc_uri_str)
            .map_err(|e| McpError::invalid_params(format!("invalid document URI: {e}"), None))?;
        match self
            .engine
            .execute(CoreOperation::GetOutline { uri: doc_uri })
        {
            CoreOperationResult::Outline(headings) => {
                let json = serde_json::to_string_pretty(&headings)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(vec![ResourceContents::TextResourceContents {
                    uri: resource_uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: json,
                    meta: None,
                }])
            }
            CoreOperationResult::Error(err) => Err(McpError::internal_error(
                format!("outline failed: {err:?}"),
                None,
            )),
            _ => Err(McpError::internal_error(
                "unexpected result from GetOutline".to_string(),
                None,
            )),
        }
    }

    fn read_symbols_resource(&self, resource_uri: &str) -> Result<Vec<ResourceContents>, McpError> {
        let query = extract_query_param(resource_uri, "query").unwrap_or_default();
        if query.is_empty() {
            return Err(McpError::invalid_params(
                "query parameter is required for symbol-search resource".to_string(),
                None,
            ));
        }
        match self.engine.execute(CoreOperation::SearchSymbols { query }) {
            CoreOperationResult::Symbols(symbols) => {
                let mapped: Vec<_> = symbols
                    .into_iter()
                    .map(|(name, uri, range)| {
                        json!({
                            "name": name,
                            "uri": uri.as_str(),
                            "range": {
                                "start": { "line": range.start.line, "character": range.start.character },
                                "end": { "line": range.end.line, "character": range.end.character }
                            }
                        })
                    })
                    .collect();
                let json = serde_json::to_string_pretty(&mapped)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(vec![ResourceContents::TextResourceContents {
                    uri: resource_uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: json,
                    meta: None,
                }])
            }
            CoreOperationResult::Error(err) => Err(McpError::internal_error(
                format!("symbol search failed: {err:?}"),
                None,
            )),
            _ => Err(McpError::internal_error(
                "unexpected result from SearchSymbols".to_string(),
                None,
            )),
        }
    }

    fn read_dependency_graph_resource(
        &self,
        resource_uri: &str,
    ) -> Result<Vec<ResourceContents>, McpError> {
        let realm =
            extract_query_param(resource_uri, "realm").unwrap_or_else(|| "default".to_string());
        let format =
            extract_query_param(resource_uri, "format").unwrap_or_else(|| "json".to_string());

        let mime = if format == "dot" {
            "text/vnd.graphviz"
        } else {
            "application/json"
        };

        match self
            .engine
            .execute(CoreOperation::DependencyGraph { realm, format })
        {
            CoreOperationResult::DependencyGraph { content, .. } => {
                Ok(vec![ResourceContents::TextResourceContents {
                    uri: resource_uri.to_string(),
                    mime_type: Some(mime.to_string()),
                    text: content,
                    meta: None,
                }])
            }
            CoreOperationResult::Error(err) => Err(McpError::internal_error(
                format!("dependency graph failed: {err:?}"),
                None,
            )),
            _ => Err(McpError::internal_error(
                "unexpected result from DependencyGraph".to_string(),
                None,
            )),
        }
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

    /// Find all references to a symbol at the given position.
    #[tool(
        name = "find-references",
        description = "Find all references to a heading or XML tag at a position."
    )]
    pub async fn find_references_tool(
        &self,
        params: Parameters<FindReferencesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let uri = match parse_file_uri(&params.0.uri) {
            Ok(uri) => uri,
            Err(err) => return Ok(tool_error(&err.code, err.message)),
        };

        let position = Range::new(
            Position::new(params.0.line, params.0.character),
            Position::new(params.0.line, params.0.character),
        );

        match self.find_references(uri, position) {
            CoreOperationResult::Locations(locations) => {
                let mut mapped: Vec<LocationDto> = locations
                    .into_iter()
                    .map(|(uri, range)| LocationDto {
                        uri: uri.as_str().to_string(),
                        range: range_to_dto(range),
                    })
                    .collect();
                mapped.sort();

                Ok(CallToolResult::structured(json!(FindReferencesResponse {
                    uri: params.0.uri,
                    locations: mapped,
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("find-references", &other)),
        }
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
        let uri = match parse_file_uri(&params.0.uri) {
            Ok(uri) => uri,
            Err(err) => return Ok(tool_error(&err.code, err.message)),
        };

        let new_name = params.0.new_name.trim().to_string();
        if new_name.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "new_name must not be empty for rename",
            ));
        }

        let position = Range::new(
            Position::new(params.0.line, params.0.character),
            Position::new(params.0.line, params.0.character),
        );

        match self.rename(uri, position, new_name) {
            CoreOperationResult::WorkspaceEdit(edits) => {
                let mut changes: Vec<DocumentEditDto> = edits
                    .into_iter()
                    .map(|(uri, text_edits)| DocumentEditDto {
                        uri: uri.as_str().to_string(),
                        edits: text_edits
                            .into_iter()
                            .map(|(range, new_text)| TextEditDto {
                                range: range_to_dto(range),
                                new_text,
                            })
                            .collect(),
                    })
                    .collect();
                changes.sort();

                Ok(CallToolResult::structured(json!(RenameResponse {
                    changes
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("rename", &other)),
        }
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
        let name = params.0.name.trim().to_string();
        if name.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for create-realm",
            ));
        }

        match self.engine.execute(CoreOperation::CreateRealm { name }) {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("create-realm", &other)),
        }
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
        let name = params.0.name.trim().to_string();
        if name.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for destroy-realm",
            ));
        }

        match self.engine.execute(CoreOperation::DestroyRealm { name }) {
            CoreOperationResult::Ok => {
                Ok(CallToolResult::structured(json!(DestroyRealmResponse {
                    success: true
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("destroy-realm", &other)),
        }
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
        let realm = params.0.realm.trim().to_string();
        if realm.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for add-root",
            ));
        }

        let root = std::path::PathBuf::from(&params.0.root);

        match self.engine.execute(CoreOperation::AddRoot { realm, root }) {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("add-root", &other)),
        }
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
        let realm = params.0.realm.trim().to_string();
        if realm.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for remove-root",
            ));
        }

        let root = std::path::PathBuf::from(&params.0.root);

        match self
            .engine
            .execute(CoreOperation::RemoveRoot { realm, root })
        {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("remove-root", &other)),
        }
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
        let realm = params.0.realm.trim().to_string();
        if realm.is_empty() {
            return Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for realm-stats",
            ));
        }

        match self.engine.execute(CoreOperation::RealmStats { realm }) {
            CoreOperationResult::RealmStats {
                name,
                root_count,
                document_count,
                heading_count,
                xml_tag_count,
                wiki_link_count,
                markdown_link_count,
            } => Ok(CallToolResult::structured(json!(RealmStatsResponse {
                name,
                root_count,
                document_count,
                heading_count,
                xml_tag_count,
                wiki_link_count,
                markdown_link_count,
            }))),
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("realm-stats", &other)),
        }
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
        let uri = match parse_file_uri(&params.0.uri) {
            Ok(uri) => uri,
            Err(err) => return Ok(tool_error(&err.code, err.message)),
        };

        match self.engine.execute(CoreOperation::ExportIndex { uri }) {
            CoreOperationResult::DocumentExport {
                uri,
                headings,
                xml_tags,
                wiki_links,
                markdown_links,
            } => {
                let headings: Vec<ExportedHeadingDto> = headings
                    .into_iter()
                    .map(|(text, level, range)| ExportedHeadingDto {
                        text,
                        level,
                        range: range_to_dto(range),
                    })
                    .collect();

                let xml_tags: Vec<ExportedXmlTagDto> = xml_tags
                    .into_iter()
                    .map(|(tag_name, range)| ExportedXmlTagDto {
                        tag_name,
                        range: range_to_dto(range),
                    })
                    .collect();

                let wiki_links: Vec<ExportedWikiLinkDto> = wiki_links
                    .into_iter()
                    .map(|(target, heading, range)| ExportedWikiLinkDto {
                        target,
                        heading,
                        range: range_to_dto(range),
                    })
                    .collect();

                let markdown_links: Vec<ExportedMarkdownLinkDto> = markdown_links
                    .into_iter()
                    .map(|(text, url, range)| ExportedMarkdownLinkDto {
                        text,
                        url,
                        range: range_to_dto(range),
                    })
                    .collect();

                Ok(CallToolResult::structured(json!(ExportIndexResponse {
                    uri: uri.as_str().to_string(),
                    headings,
                    xml_tags,
                    wiki_links,
                    markdown_links,
                })))
            }
            CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
            other => Ok(unexpected_result_error("export-index", &other)),
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

/// Extract a query parameter value from a URI string.
///
/// Performs simple string parsing (no full URL parser dependency).
fn extract_query_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}
