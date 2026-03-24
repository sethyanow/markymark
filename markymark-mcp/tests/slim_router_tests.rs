//! Integration tests for the SlimMarkymarkMcp router.
//!
//! Validates that the single `execute` tool correctly routes to all existing
//! MCP operations via operation name dispatch.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_mcp::SlimMarkymarkMcp;
use rmcp::handler::server::wrapper::Parameters;

/// Mock engine that returns canned responses for each CoreOperation variant.
struct MockEngine;

#[async_trait]
impl CoreEngine for MockEngine {
    async fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { .. } => {
                CoreOperationResult::Outline(vec!["Heading".to_string()])
            }
            CoreOperation::SearchSymbols { query, .. } => CoreOperationResult::Symbols(vec![(
                query,
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                Range::new(Position::new(0, 0), Position::new(0, 7)),
            )]),
            CoreOperation::FindReferences { .. } => CoreOperationResult::Locations(vec![(
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                Range::new(Position::new(1, 0), Position::new(1, 5)),
            )]),
            CoreOperation::Rename { new_name, .. } => CoreOperationResult::WorkspaceEdit(vec![(
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                vec![(
                    Range::new(Position::new(2, 0), Position::new(2, 7)),
                    new_name,
                )],
            )]),
            CoreOperation::CreateRealm { name } => CoreOperationResult::RealmInfo {
                name,
                root_count: 0,
                document_count: 0,
            },
            CoreOperation::DestroyRealm { .. } => CoreOperationResult::Ok,
            CoreOperation::AddRoot { realm, .. } => CoreOperationResult::RealmInfo {
                name: realm,
                root_count: 1,
                document_count: 3,
            },
            CoreOperation::RemoveRoot { realm, .. } => CoreOperationResult::RealmInfo {
                name: realm,
                root_count: 0,
                document_count: 0,
            },
            CoreOperation::RealmStats { realm, .. } => CoreOperationResult::RealmStats {
                name: realm,
                root_count: 2,
                document_count: 5,
                heading_count: 12,
                xml_tag_count: 3,
                wiki_link_count: 5,
                markdown_link_count: 3,
                structured_doc_count: 0,
                key_path_count: 0,
                duplicate_pairs: Some(1),
                total_tokens: None,
            },
            CoreOperation::ExportIndex { uri, .. } => CoreOperationResult::DocumentExport {
                uri,
                document_kind: None,
                headings: vec![],
                xml_tags: vec![],
                wiki_links: vec![],
                markdown_links: vec![],
                frontmatter: vec![],
                properties: vec![],
                content_blocks: None,
            },
            CoreOperation::SearchWorkspace { query, .. } => {
                CoreOperationResult::WorkspaceSearchResults {
                    realm: "default".to_string(),
                    query,
                    results: vec![],
                }
            }
            CoreOperation::SearchForPattern { pattern, .. } => {
                CoreOperationResult::PatternSearchResults {
                    realm: "default".to_string(),
                    pattern,
                    files_searched: 1,
                    files_skipped: 0,
                    matches: vec![],
                    truncated: false,
                }
            }
            CoreOperation::GraphAnalysis { .. } => CoreOperationResult::GraphAnalysis {
                realm: "default".to_string(),
                total_docs: 0,
                total_internal_links: 0,
                orphans: vec![],
                hubs: vec![],
                broken_links: vec![],
                clusters: None,
            },
            CoreOperation::GetDiagnostics { .. } => CoreOperationResult::Diagnostics {
                realm: "default".to_string(),
                items: vec![],
            },
            CoreOperation::ExportDocsIndex { .. } => CoreOperationResult::DocsIndexExport {
                realm: "default".to_string(),
                entries: vec!["test|root|docs".to_string()],
                doc_count: 3,
                root_count: 1,
                skipped_count: 0,
            },
            CoreOperation::RecommendDocs { query, .. } => CoreOperationResult::Recommendations {
                realm: "default".to_string(),
                query,
                results: vec![],
            },
            CoreOperation::CurationDiagnostics { .. } => CoreOperationResult::CurationReport {
                realm: "default".to_string(),
                report: markymark_core::engine::CurationReportData {
                    orphan_docs: vec![],
                    low_connectivity_docs: vec![],
                    suggestions: vec![],
                    stats: markymark_core::engine::CurationStats {
                        total_docs: 0,
                        orphan_count: 0,
                        orphan_percentage: 0.0,
                        avg_connectivity: 0.0,
                        median_connectivity: 0.0,
                        broken_link_count: 0,
                    },
                },
            },
            CoreOperation::GetContentBlocks { uri, .. } => CoreOperationResult::ContentBlocks {
                uri,
                blocks: vec![],
            },
            CoreOperation::SearchBlockText { query, .. } => CoreOperationResult::BlockTextMatches {
                realm: "default".to_string(),
                query,
                matches: vec![],
                truncated: false,
            },
            CoreOperation::EnrichDocument { uri, .. } => CoreOperationResult::EnrichmentResult {
                uri,
                sections_enriched: 0,
                was_stale: false,
                model_id: "mock".to_string(),
            },
            _ => CoreOperationResult::Error(CoreError::Message("unhandled".to_string())),
        }
    }
}

fn make_slim() -> SlimMarkymarkMcp {
    SlimMarkymarkMcp::new(Arc::new(MockEngine))
}

// ── Tool inventory ──

#[tokio::test]
async fn list_tools_returns_exactly_one_execute_tool() {
    let slim = make_slim();
    let tools = slim.list_tools();
    assert_eq!(tools.len(), 1, "slim router should expose exactly one tool");
    assert_eq!(tools[0].name.as_ref(), "execute");
}

// ── Routing: happy paths ──

#[tokio::test]
async fn routes_get_outline() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "get-outline".to_string(),
            params: serde_json::json!({ "uri": "file:///test.md" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_search_symbols() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "search-symbols".to_string(),
            params: serde_json::json!({ "query": "heading" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_search_workspace() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "search-workspace".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_search_for_pattern() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "search-for-pattern".to_string(),
            params: serde_json::json!({ "pattern": "test" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_graph_analysis() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "graph-analysis".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_get_diagnostics() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "get-diagnostics".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_export_index() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "export-index".to_string(),
            params: serde_json::json!({ "uri": "file:///test.md" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_find_references() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "find-references".to_string(),
            params: serde_json::json!({
                "uri": "file:///test.md",
                "line": 1,
                "character": 0
            }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_rename() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "rename".to_string(),
            params: serde_json::json!({
                "uri": "file:///test.md",
                "line": 1,
                "character": 0,
                "new_name": "New Heading"
            }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_create_realm() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "create-realm".to_string(),
            params: serde_json::json!({ "name": "test-realm" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_destroy_realm() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "destroy-realm".to_string(),
            params: serde_json::json!({ "name": "test-realm" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_add_root() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "add-root".to_string(),
            params: serde_json::json!({ "realm": "default", "root": "/tmp/docs" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_remove_root() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "remove-root".to_string(),
            params: serde_json::json!({ "realm": "default", "root": "/tmp/docs" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_realm_stats() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "realm-stats".to_string(),
            params: serde_json::json!({ "realm": "default" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_export_docs_index() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "export-docs-index".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_recommend_docs() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "recommend-docs".to_string(),
            params: serde_json::json!({ "query": "architecture" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_curation_diagnostics() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "curation-diagnostics".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_get_content_blocks() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "get-content-blocks".to_string(),
            params: serde_json::json!({ "uri": "file:///test.md" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_search_block_text() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "search-block-text".to_string(),
            params: serde_json::json!({ "query": "hello" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn routes_enrich_document() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "enrich-document".to_string(),
            params: serde_json::json!({ "uri": "file:///test.md" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

// ── Error handling ──

#[tokio::test]
async fn unknown_operation_returns_error() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "nonexistent-operation".to_string(),
            params: serde_json::json!({}),
        }))
        .await
        .expect("should return tool error, not MCP error");
    assert!(result.is_error.unwrap_or(false));
    let text = result.content.first().unwrap();
    let text_str = format!("{text:?}");
    assert!(
        text_str.contains("nonexistent-operation"),
        "error should mention the unknown operation name"
    );
}

#[tokio::test]
async fn invalid_params_returns_error() {
    let slim = make_slim();
    // get-outline requires a "uri" field — omitting it should fail deserialization
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "get-outline".to_string(),
            params: serde_json::json!({ "invalid_field": 42 }),
        }))
        .await
        .expect("should return tool error, not MCP error");
    assert!(result.is_error.unwrap_or(false));
}

// ── Semantic search conditional ──

#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn routes_semantic_search() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "semantic-search".to_string(),
            params: serde_json::json!({ "query": "test" }),
        }))
        .await
        .expect("should succeed");
    assert!(!result.is_error.unwrap_or(false));
}

#[cfg(not(feature = "semantic-search"))]
#[tokio::test]
async fn semantic_search_without_feature_returns_error() {
    let slim = make_slim();
    let result = slim
        .execute_tool(Parameters(markymark_mcp::ExecuteRequest {
            operation: "semantic-search".to_string(),
            params: serde_json::json!({ "query": "test" }),
        }))
        .await
        .expect("should return tool error, not MCP error");
    assert!(result.is_error.unwrap_or(false));
}
