//! Integration tests for MarkymarkMcp tool handlers.
//!
//! These tests validate the MCP tool handler layer using a MockEngine
//! that returns canned responses for each CoreOperation variant.

use std::path::Path;
use std::sync::Arc;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_mcp::dto::*;
use markymark_mcp::MarkymarkMcp;
use rmcp::handler::server::wrapper::Parameters;

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
            (_, CoreOperation::SearchSymbols { query }) => CoreOperationResult::Symbols(vec![(
                query,
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                Range::new(Position::new(0, 0), Position::new(0, 7)),
            )]),
            (_, CoreOperation::FindReferences { .. }) => CoreOperationResult::Locations(vec![(
                DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                Range::new(Position::new(1, 0), Position::new(1, 5)),
            )]),
            (_, CoreOperation::Rename { new_name, .. }) => {
                CoreOperationResult::WorkspaceEdit(vec![(
                    DocumentUri::from_file_path(Path::new("/vault/notes.md")),
                    vec![(
                        Range::new(Position::new(2, 0), Position::new(2, 7)),
                        new_name,
                    )],
                )])
            }
            (_, CoreOperation::CreateRealm { name }) => CoreOperationResult::RealmInfo {
                name,
                root_count: 0,
                document_count: 0,
            },
            (_, CoreOperation::DestroyRealm { .. }) => CoreOperationResult::Ok,
            (_, CoreOperation::AddRoot { realm, .. }) => CoreOperationResult::RealmInfo {
                name: realm,
                root_count: 1,
                document_count: 3,
            },
            (_, CoreOperation::RemoveRoot { realm, .. }) => CoreOperationResult::RealmInfo {
                name: realm,
                root_count: 0,
                document_count: 0,
            },
            (_, CoreOperation::RealmStats { realm }) => CoreOperationResult::RealmStats {
                name: realm,
                root_count: 2,
                document_count: 5,
                heading_count: 12,
                xml_tag_count: 3,
                wiki_link_count: 8,
                markdown_link_count: 4,
                structured_doc_count: 0,
                key_path_count: 0,
            },
            (_, CoreOperation::DependencyGraph { realm, format }) => {
                CoreOperationResult::DependencyGraph {
                    realm,
                    format: format.clone(),
                    content: if format == "dot" {
                        "digraph { }".to_string()
                    } else {
                        r#"{"nodes":[],"edges":[]}"#.to_string()
                    },
                }
            }
            (_, CoreOperation::ExportIndex { uri }) => CoreOperationResult::DocumentExport {
                uri: uri.clone(),
                document_kind: None,
                headings: vec![(
                    "Introduction".to_string(),
                    1,
                    Range::new(Position::new(0, 0), Position::new(0, 16)),
                )],
                xml_tags: vec![(
                    "agent".to_string(),
                    Range::new(Position::new(2, 0), Position::new(4, 8)),
                )],
                wiki_links: vec![(
                    "other-page".to_string(),
                    Some("section".to_string()),
                    Range::new(Position::new(6, 0), Position::new(6, 25)),
                )],
                markdown_links: vec![(
                    "Click here".to_string(),
                    "https://example.com".to_string(),
                    Range::new(Position::new(8, 0), Position::new(8, 35)),
                )],
            },
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
    let tools = mcp.list_tools();
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"get-outline"));
    assert!(names.contains(&"search-symbols"));
    assert!(names.contains(&"find-references"));
    assert!(names.contains(&"rename"));
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

// --- find-references tool tests ---

#[tokio::test]
async fn find_references_tool_returns_structured_locations() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .find_references_tool(Parameters(FindReferencesRequest {
            uri: "file:///vault/notes.md".to_string(),
            line: 0,
            character: 2,
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: FindReferencesResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.uri, "file:///vault/notes.md");
    assert_eq!(payload.locations.len(), 1);
    assert_eq!(payload.locations[0].range.start.line, 1);
    assert_eq!(payload.locations[0].range.start.character, 0);
    assert_eq!(payload.locations[0].range.end.line, 1);
    assert_eq!(payload.locations[0].range.end.character, 5);
}

#[tokio::test]
async fn find_references_tool_rejects_non_file_uri() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .find_references_tool(Parameters(FindReferencesRequest {
            uri: "https://example.com/notes.md".to_string(),
            line: 0,
            character: 0,
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "non_file_uri");
}

#[tokio::test]
async fn find_references_tool_maps_core_error() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::CoreError,
    }));
    let result = mcp
        .find_references_tool(Parameters(FindReferencesRequest {
            uri: "file:///vault/notes.md".to_string(),
            line: 0,
            character: 0,
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "core_error");
}

// --- rename tool tests ---

#[tokio::test]
async fn rename_tool_returns_structured_workspace_edit() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .rename_tool(Parameters(RenameRequest {
            uri: "file:///vault/notes.md".to_string(),
            line: 2,
            character: 3,
            new_name: "NewTitle".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RenameResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.changes.len(), 1);
    assert_eq!(payload.changes[0].edits.len(), 1);
    assert_eq!(payload.changes[0].edits[0].new_text, "NewTitle");
    assert_eq!(payload.changes[0].edits[0].range.start.line, 2);
    assert_eq!(payload.changes[0].edits[0].range.start.character, 0);
}

#[tokio::test]
async fn rename_tool_rejects_non_file_uri() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .rename_tool(Parameters(RenameRequest {
            uri: "https://example.com/notes.md".to_string(),
            line: 0,
            character: 0,
            new_name: "Whatever".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "non_file_uri");
}

#[tokio::test]
async fn rename_tool_rejects_empty_name() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .rename_tool(Parameters(RenameRequest {
            uri: "file:///vault/notes.md".to_string(),
            line: 0,
            character: 0,
            new_name: "   ".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

#[tokio::test]
async fn rename_tool_maps_core_error() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::CoreError,
    }));
    let result = mcp
        .rename_tool(Parameters(RenameRequest {
            uri: "file:///vault/notes.md".to_string(),
            line: 0,
            character: 0,
            new_name: "NewName".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "core_error");
}

// --- realm tool registration ---

#[test]
fn registers_realm_management_tools() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let tools = mcp.list_tools();
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"create-realm"), "missing create-realm tool");
    assert!(
        names.contains(&"destroy-realm"),
        "missing destroy-realm tool"
    );
    assert!(names.contains(&"add-root"), "missing add-root tool");
    assert!(names.contains(&"remove-root"), "missing remove-root tool");
}

// --- create-realm tool tests ---

#[tokio::test]
async fn create_realm_tool_returns_realm_info() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .create_realm_tool(Parameters(CreateRealmRequest {
            name: "test-realm".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RealmInfoResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.name, "test-realm");
    assert_eq!(payload.root_count, 0);
    assert_eq!(payload.document_count, 0);
}

#[tokio::test]
async fn create_realm_tool_rejects_empty_name() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .create_realm_tool(Parameters(CreateRealmRequest {
            name: "   ".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

// --- destroy-realm tool tests ---

#[tokio::test]
async fn destroy_realm_tool_returns_success() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .destroy_realm_tool(Parameters(DestroyRealmRequest {
            name: "old-realm".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: DestroyRealmResponse = result.into_typed().expect("typed response");
    assert!(payload.success);
}

#[tokio::test]
async fn destroy_realm_tool_rejects_empty_name() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .destroy_realm_tool(Parameters(DestroyRealmRequest {
            name: "   ".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

#[tokio::test]
async fn destroy_realm_tool_maps_core_error() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::CoreError,
    }));
    let result = mcp
        .destroy_realm_tool(Parameters(DestroyRealmRequest {
            name: "default".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "core_error");
}

// --- add-root tool tests ---

#[tokio::test]
async fn add_root_tool_returns_realm_info() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .add_root_tool(Parameters(AddRootRequest {
            realm: "my-realm".to_string(),
            root: "/vault/docs".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RealmInfoResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.name, "my-realm");
    assert_eq!(payload.root_count, 1);
    assert_eq!(payload.document_count, 3);
}

#[tokio::test]
async fn add_root_tool_rejects_empty_realm() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .add_root_tool(Parameters(AddRootRequest {
            realm: "   ".to_string(),
            root: "/vault/docs".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

// --- remove-root tool tests ---

#[tokio::test]
async fn remove_root_tool_returns_realm_info() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .remove_root_tool(Parameters(RemoveRootRequest {
            realm: "my-realm".to_string(),
            root: "/vault/docs".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RealmInfoResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.name, "my-realm");
    assert_eq!(payload.root_count, 0);
    assert_eq!(payload.document_count, 0);
}

#[tokio::test]
async fn remove_root_tool_rejects_empty_realm() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .remove_root_tool(Parameters(RemoveRootRequest {
            realm: "   ".to_string(),
            root: "/vault/docs".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

// --- realm-stats tool tests ---

#[test]
fn registers_realm_stats_and_export_index_tools() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let tools = mcp.list_tools();
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"realm-stats"), "missing realm-stats tool");
    assert!(names.contains(&"export-index"), "missing export-index tool");
}

#[tokio::test]
async fn realm_stats_tool_returns_structured_stats() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .realm_stats_tool(Parameters(RealmStatsRequest {
            realm: "default".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RealmStatsResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.name, "default");
    assert_eq!(payload.root_count, 2);
    assert_eq!(payload.document_count, 5);
    assert_eq!(payload.heading_count, 12);
    assert_eq!(payload.xml_tag_count, 3);
    assert_eq!(payload.wiki_link_count, 8);
    assert_eq!(payload.markdown_link_count, 4);
}

#[tokio::test]
async fn realm_stats_tool_rejects_empty_realm() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .realm_stats_tool(Parameters(RealmStatsRequest {
            realm: "   ".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "invalid_name");
}

#[tokio::test]
async fn realm_stats_tool_maps_core_error() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::CoreError,
    }));
    let result = mcp
        .realm_stats_tool(Parameters(RealmStatsRequest {
            realm: "default".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "core_error");
}

// --- export-index tool tests ---

#[tokio::test]
async fn export_index_tool_returns_structured_document_export() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .export_index_tool(Parameters(ExportIndexRequest {
            uri: "file:///vault/notes.md".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: ExportIndexResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.uri, "file:///vault/notes.md");
    assert_eq!(payload.headings.len(), 1);
    assert_eq!(payload.headings[0].text, "Introduction");
    assert_eq!(payload.headings[0].level, 1);
    assert_eq!(payload.xml_tags.len(), 1);
    assert_eq!(payload.xml_tags[0].tag_name, "agent");
    assert_eq!(payload.wiki_links.len(), 1);
    assert_eq!(payload.wiki_links[0].target, "other-page");
    assert_eq!(payload.wiki_links[0].heading, Some("section".to_string()));
    assert_eq!(payload.markdown_links.len(), 1);
    assert_eq!(payload.markdown_links[0].text, "Click here");
    assert_eq!(payload.markdown_links[0].url, "https://example.com");
}

#[tokio::test]
async fn export_index_tool_rejects_non_file_uri() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::Happy,
    }));
    let result = mcp
        .export_index_tool(Parameters(ExportIndexRequest {
            uri: "https://example.com/notes.md".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "non_file_uri");
}

#[tokio::test]
async fn export_index_tool_maps_core_error() {
    let mcp = MarkymarkMcp::new(Arc::new(MockEngine {
        mode: MockMode::CoreError,
    }));
    let result = mcp
        .export_index_tool(Parameters(ExportIndexRequest {
            uri: "file:///vault/notes.md".to_string(),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(true));
    let payload: ToolErrorEnvelope = result.into_typed().expect("typed error");
    assert_eq!(payload.error.code, "core_error");
}
