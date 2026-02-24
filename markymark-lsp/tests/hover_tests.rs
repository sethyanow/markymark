//! LSP hover handler integration tests.

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
async fn test_hover_on_heading() {
    // Cursor on heading -> should return markdown with heading info
    // Line 2: "## Introduction"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 5), // on "Introduction"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on heading should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("Introduction"),
                "hover content should mention the heading text"
            );
        }
        HoverContents::Scalar(MarkedString::String(s)) => {
            assert!(s.contains("Introduction"));
        }
        HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
            assert!(ls.value.contains("Introduction"));
        }
        HoverContents::Array(arr) => {
            let text: String = arr
                .iter()
                .map(|m| match m {
                    MarkedString::String(s) => s.clone(),
                    MarkedString::LanguageString(ls) => ls.value.clone(),
                })
                .collect();
            assert!(text.contains("Introduction"));
        }
    }
}

#[tokio::test]
async fn test_hover_on_wiki_link() {
    // Cursor on [[other-page]] -> should return info about target
    // Line 4: "See [[other-page]] for details."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 8), // inside "other-page"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link should return hover info about the target"
    );
}

#[tokio::test]
async fn test_hover_on_plain_text_returns_none() {
    // Cursor on plain text -> None
    // Line 4: "See [[other-page]] for details." -- on "for"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(result.is_none(), "hover on plain text should return None");
}

#[tokio::test]
async fn test_hover_on_wiki_link_with_heading() {
    // Cursor on [[other-page#details]] -> should return hover info about the heading target
    // Line 6: "Also check [[other-page#details]] and [[#introduction]]."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(6, 20), // inside "other-page#details"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link with heading fragment should return info"
    );
}

// ---------------------------------------------------------------------------
// XML tag hover
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_hover_on_xml_tag() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/xml-doc.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/xml-doc.md").unwrap();
        state.open_document(
            core_uri,
            "<agent priority=\"high\">\n\ncontent\n\n</agent>\n".to_string(),
        );
    }

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: xml_uri.clone(),
            },
            position: Position::new(0, 3), // inside "<agent"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on XML tag should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("agent"),
                "hover should mention the tag name; got: {}",
                markup.value
            );
        }
        _ => panic!("expected markup hover content"),
    }
}

#[tokio::test]
async fn test_hover_on_xml_tag_shows_attributes() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/attrs.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/attrs.md").unwrap();
        state.open_document(
            core_uri,
            "<goal priority=\"high\" scope=\"global\">\n\nwin\n\n</goal>\n".to_string(),
        );
    }

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: xml_uri.clone(),
            },
            position: Position::new(0, 3), // inside "<goal"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on XML tag with attributes should show info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            // Blob path does not preserve per-tag attributes (acceptable trade-off
            // from B-7.2). Verify the tag name and workspace stats are present.
            assert!(
                markup.value.contains("<goal>"),
                "hover should show tag name; got: {}",
                markup.value
            );
            assert!(
                markup.value.contains("Occurrences in workspace: **1**"),
                "hover should show occurrence count; got: {}",
                markup.value
            );
        }
        _ => panic!("expected markup hover content"),
    }
}

#[tokio::test]
async fn test_hover_on_xml_tag_shows_workspace_usage_stats() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_a: Uri = "file:///workspace/xml-a.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let uri_a = DocumentUri::new("file:///workspace/xml-a.md").unwrap();
        let uri_b = DocumentUri::new("file:///workspace/xml-b.md").unwrap();
        let uri_c = DocumentUri::new("file:///workspace/xml-c.md").unwrap();

        state.open_document(
            uri_a,
            "<agent priority=\"high\" scope=\"global\">\n\na\n\n</agent>\n".to_string(),
        );
        state.open_document(
            uri_b,
            "<agent priority=\"low\">\n\nb\n\n</agent>\n".to_string(),
        );
        state.open_document(
            uri_c,
            "<task priority=\"high\">\n\nc\n\n</task>\n".to_string(),
        );
    }

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: xml_a },
            position: Position::new(0, 2), // inside "<agent"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on XML tag should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("Occurrences in workspace: **2**"),
                "hover should show workspace count; got: {}",
                markup.value
            );
            assert!(
                markup.value.contains("Documents with this tag: **2**"),
                "hover should show document count; got: {}",
                markup.value
            );
            // Blob path does not preserve per-tag attributes (acceptable trade-off
            // from B-7.2), so attribute frequency stats are empty.
        }
        _ => panic!("expected markup hover content"),
    }
}

#[tokio::test]
async fn test_hover_on_unclosed_xml_tag_shows_warning() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let xml_uri: Uri = "file:///workspace/unclosed.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/unclosed.md").unwrap();
        state.open_document(core_uri, "<agent priority=\"high\">\n".to_string());
    }

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: xml_uri },
            position: Position::new(0, 2), // inside "<agent"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on XML tag should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("Warning: unclosed tag"),
                "hover should warn for unclosed tags; got: {}",
                markup.value
            );
        }
        _ => panic!("expected markup hover content"),
    }
}

// ---------------------------------------------------------------------------
// Structured document key hover
// ---------------------------------------------------------------------------

/// Helper to extract markup value from hover result.
fn extract_hover_markdown(hover: Hover) -> String {
    match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        _ => panic!("expected markup hover content"),
    }
}

#[tokio::test]
async fn test_hover_on_json_key() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let json_uri: Uri = "file:///workspace/config.json".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/config.json").unwrap();
        state.open_document(
            core_uri,
            "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n".to_string(),
        );
    }

    // Hover on "host" key (line 2, col ~5)
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: json_uri.clone(),
            },
            position: Position::new(2, 5), // inside "host"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on JSON key should return hover info"
    );
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("database.host"),
        "should show full path; got: {}",
        markdown
    );
    assert!(
        markdown.contains("String"),
        "should show value type; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Json"),
        "should show document format; got: {}",
        markdown
    );
}

#[tokio::test]
async fn test_hover_on_yaml_key() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let yaml_uri: Uri = "file:///workspace/config.yaml".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/config.yaml").unwrap();
        state.open_document(core_uri, "server:\n  port: 8080\n".to_string());
    }

    // Hover on "port" key (line 1, col 3)
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: yaml_uri.clone(),
            },
            position: Position::new(1, 3), // inside "port"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on YAML key should return hover info"
    );
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("server.port"),
        "should show full path; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Number"),
        "should show value type; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Yaml"),
        "should show document format; got: {}",
        markdown
    );
}

#[tokio::test]
async fn test_hover_on_toml_key() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let toml_uri: Uri = "file:///workspace/config.toml".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/config.toml").unwrap();
        state.open_document(core_uri, "[package]\nname = \"myapp\"\n".to_string());
    }

    // Hover on "name" key (line 1, col 2)
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: toml_uri.clone(),
            },
            position: Position::new(1, 2), // inside "name"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on TOML key should return hover info"
    );
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("package.name"),
        "should show full path; got: {}",
        markdown
    );
    assert!(
        markdown.contains("String"),
        "should show value type; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Toml"),
        "should show document format; got: {}",
        markdown
    );
}

#[tokio::test]
async fn test_hover_on_json_non_key_returns_none() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let json_uri: Uri = "file:///workspace/empty.json".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/empty.json").unwrap();
        state.open_document(core_uri, "{\n  \"key\": \"value\"\n}\n".to_string());
    }

    // Hover on the opening brace (line 0, col 0) -- not on a key
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: json_uri.clone(),
            },
            position: Position::new(0, 0),
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_none(),
        "hover on non-key position in JSON should return None"
    );
}

#[tokio::test]
async fn test_hover_on_json_object_key() {
    let (service, _socket, _, _) = setup_workspace().await;
    let backend = service.inner();

    let json_uri: Uri = "file:///workspace/nested.json".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///workspace/nested.json").unwrap();
        state.open_document(
            core_uri,
            "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n".to_string(),
        );
    }

    // Hover on "database" key (line 1, col 4) - an Object-typed key
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: json_uri.clone(),
            },
            position: Position::new(1, 4), // inside "database"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on JSON object key should return hover info"
    );
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("database"),
        "should show key path; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Object"),
        "should show Object type; got: {}",
        markdown
    );
    assert!(
        markdown.contains("Depth:** 0"),
        "root key should be depth 0; got: {}",
        markdown
    );
}

// =======================================================================
// Code span hover tests
// =======================================================================

#[tokio::test]
async fn test_hover_on_code_span() {
    let (service, _socket) = create_service();
    let backend = service.inner();

    let uri: Uri = "file:///ws/api.md".parse().unwrap();
    {
        let mut state = backend.state().write().await;
        let core_uri = DocumentUri::new("file:///ws/api.md").unwrap();
        // Line 0: "# API"
        // Line 1: ""
        // Line 2: "Use `HashMap` for lookups."
        //          01234567890123
        //              ^--- backtick at col 4, text "HashMap" at cols 5-11, closing backtick at col 12
        state.open_document(
            core_uri,
            "# API\n\nUse `HashMap` for lookups.\n".to_string(),
        );
    }

    // Hover on "HashMap" text (line 2, col 7 — inside the code span)
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position::new(2, 7),
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on code span should return hover info"
    );
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("HashMap"),
        "hover should mention code span text; got: {}",
        markdown
    );
    assert!(
        markdown.contains("inline code span"),
        "hover should identify as code span; got: {}",
        markdown
    );
}

#[tokio::test]
async fn test_hover_on_code_span_shows_cross_doc_refs() {
    let (service, _socket) = create_service();
    let backend = service.inner();

    {
        let mut state = backend.state().write().await;
        state.open_document(
            DocumentUri::new("file:///ws/a.md").unwrap(),
            "# Doc A\n\nUse `Option` here.\n".to_string(),
        );
        state.open_document(
            DocumentUri::new("file:///ws/b.md").unwrap(),
            "# Doc B\n\nAlso `Option` there.\n".to_string(),
        );
    }

    // Hover on "Option" in doc A (line 2, col 7)
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: "file:///ws/a.md".parse().unwrap(),
            },
            position: Position::new(2, 7),
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(result.is_some(), "hover should return info");
    let markdown = extract_hover_markdown(result.unwrap());
    assert!(
        markdown.contains("Referenced in 2 documents"),
        "should show cross-doc reference count; got: {}",
        markdown
    );
}
