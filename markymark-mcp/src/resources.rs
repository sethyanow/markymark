//! MCP resource handler implementations for MarkymarkMcp.
//!
//! Provides resource templates and read_resource dispatch for:
//! - `markymark://outline/{uri}` — document heading outline
//! - `markymark://symbols?query={query}` — symbol search
//! - `markymark://dependency-graph?realm={realm}&format={format}` — link graph

use markymark_core::engine::{CoreOperation, CoreOperationResult};
use markymark_core::DocumentUri;
use rmcp::{
    model::{RawResourceTemplate, ResourceContents, ResourceTemplate},
    ErrorData as McpError,
};
use serde_json::json;

use crate::MarkymarkMcp;

impl MarkymarkMcp {
    /// Return the MCP resource templates this server advertises.
    pub fn resource_templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                raw: RawResourceTemplate {
                    uri_template: "markymark://outline/{uri}?realm={realm}".to_string(),
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
                    uri_template: "markymark://symbols?query={query}&realm={realm}".to_string(),
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
        // Strip any query params (e.g. ?realm=x) from the doc URI before parsing.
        let doc_uri_str = doc_uri_str.split('?').next().unwrap_or(doc_uri_str);
        let realm = extract_query_param(resource_uri, "realm");
        let doc_uri = DocumentUri::new(doc_uri_str)
            .map_err(|e| McpError::invalid_params(format!("invalid document URI: {e}"), None))?;
        match self.engine.execute(CoreOperation::GetOutline {
            uri: doc_uri,
            realm,
        }) {
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
        let realm = extract_query_param(resource_uri, "realm");
        match self
            .engine
            .execute(CoreOperation::SearchSymbols { query, realm })
        {
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
}

/// Extract a query parameter value from a URI string.
///
/// Performs simple string parsing (no full URL parser dependency).
pub(crate) fn extract_query_param(uri: &str, key: &str) -> Option<String> {
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
