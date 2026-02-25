//! LSP document_symbol handler integration tests.

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

/// Helper: create a Backend pre-loaded with test documents.
///
/// Returns the service, socket, main URI, and other-page URI.
async fn setup_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_main: Uri = "file:///workspace/main.md".parse().unwrap();
    let uri_other: Uri = "file:///workspace/other-page.md".parse().unwrap();

    let main_text = concat!(
        "# Main Document\n",
        "\n",
        "## Introduction\n",
        "\n",
        "See [[other-page]] for details.\n",
        "\n",
        "Also check [[other-page#details]] and [[#introduction]].\n",
        "\n",
        "A markdown link: [intro](#introduction)\n",
    );

    let other_text = concat!(
        "# Other Page\n",
        "\n",
        "## Details\n",
        "\n",
        "Some detailed content here.\n",
    );

    // Populate state via the Backend's state handle
    {
        let mut state = backend.state().write().await;
        let core_main = DocumentUri::new("file:///workspace/main.md").unwrap();
        let core_other = DocumentUri::new("file:///workspace/other-page.md").unwrap();
        state.open_document(core_main, main_text.to_string());
        state.open_document(core_other, other_text.to_string());
    }

    (service, socket, uri_main, uri_other)
}

#[tokio::test]
async fn test_document_symbol_returns_heading_hierarchy() {
    // Document with H1>H2 hierarchy -> nested DocumentSymbol array
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: uri_main.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for a document with headings"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            // Should have at least the H1 "Main Document"
            assert!(
                !symbols.is_empty(),
                "should return at least one top-level symbol"
            );
            let h1 = &symbols[0];
            assert_eq!(h1.name, "Main Document");
            assert_eq!(h1.kind, SymbolKind::STRING);
            // H1 should have H2 "Introduction" as a child
            assert!(
                h1.children.as_ref().is_some_and(|c| !c.is_empty()),
                "H1 should have child symbols for nested headings"
            );
            let children = h1.children.as_ref().unwrap();
            assert_eq!(children[0].name, "Introduction");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            // Flat response is also acceptable; just verify headings present
            assert!(
                symbols.len() >= 2,
                "flat response should include at least 2 heading symbols"
            );
        }
    }
}

#[tokio::test]
async fn test_document_symbol_empty_document() {
    // Empty document -> empty or None
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    // Add an empty document
    let empty_uri: Uri = "file:///workspace/empty.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/empty.md").unwrap();
        state.open_document(core_uri, String::new());
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: empty_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    // Empty doc should return None or empty list
    let is_empty = match &result {
        None => true,
        Some(DocumentSymbolResponse::Nested(s)) => s.is_empty(),
        Some(DocumentSymbolResponse::Flat(s)) => s.is_empty(),
    };
    assert!(
        is_empty,
        "document_symbol for empty document should return empty or None"
    );
}

#[tokio::test]
async fn test_document_symbol_flat_headings() {
    // Document with multiple H1s -> flat list of top-level symbols
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let flat_uri: Uri = "file:///workspace/flat.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/flat.md").unwrap();
        state.open_document(core_uri, "# First\n\n# Second\n\n# Third\n".to_string());
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: flat_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for a document with headings"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            assert_eq!(symbols.len(), 3, "should have 3 top-level H1 symbols");
            assert_eq!(symbols[0].name, "First");
            assert_eq!(symbols[1].name, "Second");
            assert_eq!(symbols[2].name, "Third");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            assert_eq!(
                symbols.len(),
                3,
                "should have 3 heading symbols in flat mode"
            );
        }
    }
}

#[tokio::test]
async fn test_document_symbol_other_page() {
    // Verify symbols for other-page.md (H1 "Other Page" > H2 "Details")
    let (service, _socket, _, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: uri_other.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "document_symbol should return symbols for other-page.md"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            assert_eq!(symbols.len(), 1, "should have 1 top-level H1");
            assert_eq!(symbols[0].name, "Other Page");
            let children = symbols[0].children.as_ref().unwrap();
            assert_eq!(children.len(), 1, "H1 should have 1 child H2");
            assert_eq!(children[0].name, "Details");
        }
        DocumentSymbolResponse::Flat(symbols) => {
            assert!(symbols.len() >= 2, "should have at least 2 symbols");
        }
    }
}

// ---------------------------------------------------------------------------
// XML tag document symbols
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_document_symbol_includes_xml_tags() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/xml-doc.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/xml-doc.md").unwrap();
        state.open_document(
            core_uri,
            "# Config\n\n<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n".to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: xml_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(result.is_some(), "should return symbols");
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            // Should have heading + XML tags
            // Headings are nested, XML tags should appear as top-level symbols
            let all_names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
            assert!(
                all_names.contains(&"<agent>"),
                "should include <agent> XML tag in symbols, got: {:?}",
                all_names
            );
            assert!(
                all_names.contains(&"<goal>"),
                "should include <goal> XML tag in symbols, got: {:?}",
                all_names
            );
        }
        _ => panic!("expected nested response"),
    }
}

#[tokio::test]
async fn test_document_symbol_xml_only_document() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/xml-only.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/xml-only.md").unwrap();
        state.open_document(
            core_uri,
            "<agent>\n\ncontent\n\n</agent>\n\n<br/>\n".to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: xml_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "XML-only document should still return symbols"
    );
    match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => {
            assert!(
                !symbols.is_empty(),
                "should have at least one XML tag symbol"
            );
            // Check we have the agent tag
            let has_agent = symbols.iter().any(|s| s.name == "<agent>");
            assert!(has_agent, "should contain <agent> symbol");
        }
        _ => panic!("expected nested response"),
    }
}

#[tokio::test]
async fn test_document_symbol_nests_xml_tags_by_range() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/xml-nested.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/xml-nested.md").unwrap();
        state.open_document(
            core_uri,
            concat!(
                "<agent>\n",
                "  <goal>win</goal>\n",
                "  <task>\n",
                "    <step>one</step>\n",
                "  </task>\n",
                "</agent>\n",
            )
            .to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: xml_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "nested XML document should return symbols"
    );

    let symbols = match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        _ => panic!("expected nested response"),
    };

    let agent = symbols
        .iter()
        .find(|s| s.name == "<agent>")
        .expect("top-level <agent> symbol should exist");
    let agent_children = agent
        .children
        .as_ref()
        .expect("<agent> should contain nested XML symbols");
    assert!(
        agent_children.iter().any(|s| s.name == "<goal>"),
        "<agent> should include <goal> as child"
    );

    let task = agent_children
        .iter()
        .find(|s| s.name == "<task>")
        .expect("<agent> should include <task> as child");
    let task_children = task
        .children
        .as_ref()
        .expect("<task> should contain nested XML symbols");
    assert!(
        task_children.iter().any(|s| s.name == "<step>"),
        "<task> should include <step> as child"
    );
}

// ---------------------------------------------------------------------------
// Logseq-flavored markdown (headings inside list items: `- # Heading`)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_document_symbol_logseq_headings() {
    // Logseq prefixes headings with list markers: `- # Heading`
    // These should be detected as headings in the document symbol response.
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let logseq_uri: Uri = "file:///workspace/logseq.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/logseq.md").unwrap();
        state.open_document(
            core_uri,
            concat!(
                "# Main Title\n",
                "- ## Section A\n",
                "\t- some content under A\n",
                "- ## Section B\n",
                "\t- content under B\n",
            )
            .to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: logseq_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(result.is_some(), "logseq document should return symbols");

    let symbols = match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        _ => panic!("expected nested response"),
    };

    // Should have "Main Title" as H1 at top level
    assert!(
        symbols.iter().any(|s| s.name == "Main Title"),
        "should find standard H1 heading, got: {:?}",
        symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // H1 should contain Logseq-style H2 children
    let h1 = symbols.iter().find(|s| s.name == "Main Title").unwrap();
    let children = h1
        .children
        .as_ref()
        .expect("H1 should have children from Logseq-style headings");

    let child_names: Vec<&str> = children.iter().map(|s| s.name.as_str()).collect();
    assert!(
        child_names.contains(&"Section A"),
        "H1 should contain Logseq H2 'Section A', got: {:?}",
        child_names
    );
    assert!(
        child_names.contains(&"Section B"),
        "H1 should contain Logseq H2 'Section B', got: {:?}",
        child_names
    );
}

#[tokio::test]
async fn test_document_symbol_logseq_deep_nesting() {
    // Test deeper Logseq heading nesting: H1 > H2 > H3
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let logseq_uri: Uri = "file:///workspace/logseq-deep.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/logseq-deep.md").unwrap();
        state.open_document(
            core_uri,
            concat!(
                "- # WIP system\n",
                "- ## Projects\n",
                "- ### Active\n",
                "\t- project details\n",
                "- ### Backlog\n",
                "\t- more stuff\n",
                "- ## Done\n",
            )
            .to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: logseq_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "logseq deep-nesting document should return symbols"
    );

    let symbols = match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        _ => panic!("expected nested response"),
    };

    // Top level: "WIP system" (H1)
    assert_eq!(symbols.len(), 1, "should have 1 top-level H1");
    assert_eq!(symbols[0].name, "WIP system");

    // H1 children: "Projects" (H2), "Done" (H2)
    let h1_children = symbols[0]
        .children
        .as_ref()
        .expect("H1 should have H2 children");
    let h1_child_names: Vec<&str> = h1_children.iter().map(|s| s.name.as_str()).collect();
    assert!(
        h1_child_names.contains(&"Projects"),
        "H1 should contain 'Projects', got: {:?}",
        h1_child_names
    );
    assert!(
        h1_child_names.contains(&"Done"),
        "H1 should contain 'Done', got: {:?}",
        h1_child_names
    );

    // "Projects" H2 children: "Active" (H3), "Backlog" (H3)
    let projects = h1_children.iter().find(|s| s.name == "Projects").unwrap();
    let proj_children = projects
        .children
        .as_ref()
        .expect("Projects should have H3 children");
    let proj_child_names: Vec<&str> = proj_children.iter().map(|s| s.name.as_str()).collect();
    assert!(
        proj_child_names.contains(&"Active"),
        "Projects should contain 'Active', got: {:?}",
        proj_child_names
    );
    assert!(
        proj_child_names.contains(&"Backlog"),
        "Projects should contain 'Backlog', got: {:?}",
        proj_child_names
    );
}

// ---------------------------------------------------------------------------
// Structured document symbols (JSON, YAML, TOML, etc.)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_document_symbol_json_file() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let json_uri: Uri = "file:///workspace/config.json".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/config.json").unwrap();
        state.open_document(
            core_uri,
            r#"{"database": {"host": "localhost", "port": 5432}, "debug": true}"#.to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: json_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(
        result.is_some(),
        "JSON document should return symbols for its keys"
    );

    let symbols = match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        _ => panic!("expected nested response"),
    };

    // Root keys: "database" (Object) and "debug" (Boolean)
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"database"),
        "should contain 'database' key, got: {:?}",
        names
    );
    assert!(
        names.contains(&"debug"),
        "should contain 'debug' key, got: {:?}",
        names
    );

    // "database" should have children "host" and "port"
    let db = symbols.iter().find(|s| s.name == "database").unwrap();
    assert_eq!(db.kind, SymbolKind::OBJECT);
    let db_children = db.children.as_ref().expect("database should have children");
    let child_names: Vec<&str> = db_children.iter().map(|s| s.name.as_str()).collect();
    assert!(
        child_names.contains(&"host"),
        "database should contain 'host', got: {:?}",
        child_names
    );
    assert!(
        child_names.contains(&"port"),
        "database should contain 'port', got: {:?}",
        child_names
    );

    // "debug" should be a leaf (Boolean)
    let debug = symbols.iter().find(|s| s.name == "debug").unwrap();
    assert_eq!(debug.kind, SymbolKind::BOOLEAN);
    assert!(
        debug.children.is_none() || debug.children.as_ref().unwrap().is_empty(),
        "debug should be a leaf symbol"
    );
}

#[tokio::test]
async fn test_document_symbol_yaml_file() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let yaml_uri: Uri = "file:///workspace/config.yaml".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/config.yaml").unwrap();
        state.open_document(
            core_uri,
            "server:\n  host: localhost\n  port: 8080\nlogging: true\n".to_string(),
        );
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: yaml_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    assert!(result.is_some(), "YAML document should return symbols");

    let symbols = match result.unwrap() {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        _ => panic!("expected nested response"),
    };

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"server"), "should contain 'server'");
    assert!(names.contains(&"logging"), "should contain 'logging'");

    let server = symbols.iter().find(|s| s.name == "server").unwrap();
    assert_eq!(server.kind, SymbolKind::OBJECT);
    let server_children = server
        .children
        .as_ref()
        .expect("server should have children");
    assert!(
        server_children.iter().any(|s| s.name == "host"),
        "server should contain 'host'"
    );
    assert!(
        server_children.iter().any(|s| s.name == "port"),
        "server should contain 'port'"
    );
}

#[tokio::test]
async fn test_document_symbol_empty_json() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let json_uri: Uri = "file:///workspace/empty.json".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/empty.json").unwrap();
        state.open_document(core_uri, "{}".to_string());
    }

    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier {
            uri: json_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = backend.document_symbol(params).await.unwrap();
    let is_empty = match &result {
        None => true,
        Some(DocumentSymbolResponse::Nested(s)) => s.is_empty(),
        Some(DocumentSymbolResponse::Flat(s)) => s.is_empty(),
    };
    assert!(
        is_empty,
        "empty JSON object should return empty or None symbols"
    );
}
