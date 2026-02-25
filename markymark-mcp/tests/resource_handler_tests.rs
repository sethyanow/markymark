//! Integration tests for MarkymarkMcp MCP resource handlers.
//!
//! These tests validate the MCP resource layer: list_resource_templates and
//! read_resource. Uses a MockEngine for deterministic responses.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_mcp::MarkymarkMcp;
use rmcp::model::ResourceContents;
use rmcp::ServerHandler;

struct MockEngine {
    captured_realm: Mutex<Option<Option<String>>>,
}

impl Default for MockEngine {
    fn default() -> Self {
        Self {
            captured_realm: Mutex::new(None),
        }
    }
}

#[async_trait]
impl CoreEngine for MockEngine {
    async fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { realm, .. } => {
                let mut captured = self.captured_realm.lock().expect("mutex poisoned");
                *captured = Some(realm);
                CoreOperationResult::Outline(vec!["Introduction".to_string(), "Usage".to_string()])
            }
            CoreOperation::SearchSymbols { query, .. } => CoreOperationResult::Symbols(vec![(
                format!("match-{query}"),
                DocumentUri::from_file_path(Path::new("/vault/a.md")),
                Range::new(Position::new(0, 0), Position::new(0, 10)),
            )]),
            CoreOperation::ExportIndex { uri, .. } => CoreOperationResult::DocumentExport {
                uri: uri.clone(),
                document_kind: None,
                headings: vec![(
                    "Introduction".to_string(),
                    1,
                    Range::new(Position::new(0, 0), Position::new(0, 16)),
                )],
                xml_tags: vec![],
                wiki_links: vec![],
                markdown_links: vec![],
                frontmatter: vec![],
                properties: vec![],
            },
            CoreOperation::DependencyGraph { realm, format } => {
                let content = if format == "dot" {
                    "digraph { }".to_string()
                } else {
                    r#"{"nodes":[],"edges":[]}"#.to_string()
                };
                CoreOperationResult::DependencyGraph {
                    realm,
                    format,
                    content,
                }
            }
            _ => CoreOperationResult::Error(CoreError::Message("not mocked".to_string())),
        }
    }
}

fn make_mcp() -> MarkymarkMcp {
    MarkymarkMcp::new(Arc::new(MockEngine::default()))
}

fn make_mcp_with_engine() -> (MarkymarkMcp, Arc<MockEngine>) {
    let engine = Arc::new(MockEngine::default());
    (MarkymarkMcp::new(engine.clone()), engine)
}

// --- list_resource_templates ---

#[test]
fn lists_three_resource_templates() {
    let mcp = make_mcp();
    let templates = mcp.resource_templates();
    assert_eq!(templates.len(), 3, "expected 3 resource templates");
}

#[test]
fn outline_template_uses_correct_uri_pattern() {
    let mcp = make_mcp();
    let templates = mcp.resource_templates();
    let outline = templates
        .iter()
        .find(|t| t.raw.name == "document-outline")
        .expect("missing document-outline template");
    assert!(
        outline.raw.uri_template.contains("realm"),
        "outline template should expose realm param (got: {})",
        outline.raw.uri_template
    );
    assert_eq!(
        outline.raw.mime_type.as_deref(),
        Some("application/json"),
        "wrong outline MIME type"
    );
}

#[test]
fn symbols_template_uses_correct_uri_pattern() {
    let mcp = make_mcp();
    let templates = mcp.resource_templates();
    let symbols = templates
        .iter()
        .find(|t| t.raw.name == "symbol-search")
        .expect("missing symbol-search template");
    assert!(
        symbols.raw.uri_template.contains("query"),
        "symbols template should include query parameter"
    );
}

#[test]
fn dependency_graph_template_uses_correct_uri_pattern() {
    let mcp = make_mcp();
    let templates = mcp.resource_templates();
    let graph = templates
        .iter()
        .find(|t| t.raw.name == "dependency-graph")
        .expect("missing dependency-graph template");
    assert!(
        graph.raw.uri_template.contains("realm"),
        "graph template should include realm parameter"
    );
    assert!(
        graph.raw.uri_template.contains("format"),
        "graph template should include format parameter"
    );
}

// --- resource template realm params ---

#[test]
fn symbols_template_includes_realm_param() {
    let mcp = make_mcp();
    let templates = mcp.resource_templates();
    let symbols = templates
        .iter()
        .find(|t| t.raw.name == "symbol-search")
        .expect("missing symbol-search template");
    assert!(
        symbols.raw.uri_template.contains("realm"),
        "symbol-search template should expose realm param (got: {})",
        symbols.raw.uri_template
    );
}

// --- read_resource: outline ---

#[tokio::test]
async fn read_outline_resource_returns_json() {
    let mcp = make_mcp();
    let result = mcp
        .read_resource_sync("markymark://outline/file:///vault/notes.md")
        .await;
    let contents = result.expect("should succeed");
    assert_eq!(contents.len(), 1, "expected single resource content");
    match &contents[0] {
        ResourceContents::TextResourceContents {
            text, mime_type, ..
        } => {
            assert_eq!(mime_type.as_deref(), Some("application/json"));
            let parsed: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
            let headings = parsed.as_array().expect("should be array");
            assert_eq!(headings.len(), 2);
            assert_eq!(headings[0].as_str(), Some("Introduction"));
            assert_eq!(headings[1].as_str(), Some("Usage"));
        }
        _ => panic!("expected TextResourceContents"),
    }
}

#[tokio::test]
async fn read_outline_resource_with_realm_query_succeeds() {
    let (mcp, engine) = make_mcp_with_engine();
    // realm query param must not bleed into the document URI
    let result = mcp
        .read_resource_sync("markymark://outline/file:///vault/notes.md?realm=custom")
        .await;
    assert!(
        result.is_ok(),
        "outline resource should succeed when realm query param is present; got: {:?}",
        result.err()
    );
    let captured = engine
        .captured_realm
        .lock()
        .expect("mutex poisoned")
        .clone();
    assert_eq!(
        captured,
        Some(Some("custom".to_string())),
        "realm should be forwarded to GetOutline"
    );
}

#[tokio::test]
async fn read_outline_resource_with_percent_encoded_realm_query_decodes_value() {
    let (mcp, engine) = make_mcp_with_engine();
    let result = mcp
        .read_resource_sync("markymark://outline/file:///vault/notes.md?realm=custom%20realm")
        .await;
    assert!(
        result.is_ok(),
        "outline resource should succeed when percent-encoded realm is present; got: {:?}",
        result.err()
    );
    let captured = engine
        .captured_realm
        .lock()
        .expect("mutex poisoned")
        .clone();
    assert_eq!(
        captured,
        Some(Some("custom realm".to_string())),
        "percent-encoded realm should be decoded before forwarding"
    );
}

// --- read_resource: symbols ---

#[tokio::test]
async fn read_symbols_resource_returns_json() {
    let mcp = make_mcp();
    let result = mcp
        .read_resource_sync("markymark://symbols?query=test")
        .await;
    let contents = result.expect("should succeed");
    assert_eq!(contents.len(), 1);
    match &contents[0] {
        ResourceContents::TextResourceContents {
            text, mime_type, ..
        } => {
            assert_eq!(mime_type.as_deref(), Some("application/json"));
            let parsed: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
            let symbols = parsed.as_array().expect("should be array");
            assert_eq!(symbols.len(), 1);
        }
        _ => panic!("expected TextResourceContents"),
    }
}

#[tokio::test]
async fn read_symbols_resource_with_realm_query_succeeds() {
    let mcp = make_mcp();
    let result = mcp
        .read_resource_sync("markymark://symbols?query=test&realm=custom")
        .await;
    assert!(
        result.is_ok(),
        "symbols resource should succeed when realm query param is present; got: {:?}",
        result.err()
    );
}

// --- read_resource: dependency-graph ---

#[tokio::test]
async fn read_dependency_graph_json_resource() {
    let mcp = make_mcp();
    let result = mcp
        .read_resource_sync("markymark://dependency-graph?realm=default&format=json")
        .await;
    let contents = result.expect("should succeed");
    assert_eq!(contents.len(), 1);
    match &contents[0] {
        ResourceContents::TextResourceContents {
            text, mime_type, ..
        } => {
            assert_eq!(mime_type.as_deref(), Some("application/json"));
            let _: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        }
        _ => panic!("expected TextResourceContents"),
    }
}

#[tokio::test]
async fn read_dependency_graph_dot_resource() {
    let mcp = make_mcp();
    let result = mcp
        .read_resource_sync("markymark://dependency-graph?realm=default&format=dot")
        .await;
    let contents = result.expect("should succeed");
    assert_eq!(contents.len(), 1);
    match &contents[0] {
        ResourceContents::TextResourceContents {
            text, mime_type, ..
        } => {
            assert_eq!(mime_type.as_deref(), Some("text/vnd.graphviz"));
            assert!(
                text.contains("digraph"),
                "dot output should contain 'digraph'"
            );
        }
        _ => panic!("expected TextResourceContents"),
    }
}

// --- read_resource: unknown URI ---

#[tokio::test]
async fn read_unknown_resource_returns_error() {
    let mcp = make_mcp();
    let result = mcp.read_resource_sync("markymark://unknown/foo").await;
    assert!(result.is_err(), "unknown resource URI should fail");
}

// --- capabilities ---

#[test]
fn server_info_enables_resources() {
    let mcp = make_mcp();
    let info = mcp.get_info();
    assert!(
        info.capabilities.resources.is_some(),
        "server should advertise resource capability"
    );
}
