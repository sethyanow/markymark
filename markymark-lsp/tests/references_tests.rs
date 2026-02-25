//! LSP references handler integration tests.

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
        state.open_document(core_main, main_text.to_string()).await;
        state
            .open_document(core_other, other_text.to_string())
            .await;
    }

    (service, socket, uri_main, uri_other)
}

#[tokio::test]
async fn test_references_for_heading() {
    // Cursor on "## Introduction" -> should return all wiki links referencing "introduction"
    // Line 2: "## Introduction"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 5), // on "Introduction"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for heading with incoming links should return locations"
    );
    let locs = result.unwrap();
    // [[#introduction]] on line 6 and [intro](#introduction) on line 8
    assert!(
        locs.len() >= 2,
        "should find at least 2 references to introduction: found {}",
        locs.len()
    );
}

#[tokio::test]
async fn test_references_for_heading_across_docs() {
    // Heading in other-page.md referenced from main.md via [[other-page#details]]
    // Cursor on "## Details" in other-page.md -> line 2 of other-page.md
    let (service, _socket, _uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_other.clone(),
            },
            position: Position::new(2, 5), // on "Details"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for heading referenced from another doc should return locations"
    );
    let locs = result.unwrap();
    // main.md has [[other-page#details]] on line 6
    assert!(
        !locs.is_empty(),
        "should find at least 1 cross-document reference to details"
    );
    // Verify at least one reference points to main.md
    let main_uri_str = "file:///workspace/main.md";
    assert!(
        locs.iter().any(|l| l.uri.as_str() == main_uri_str),
        "should include a reference from main.md"
    );
}

#[tokio::test]
async fn test_references_for_heading_no_refs() {
    // Cursor on "# Other Page" heading which has no incoming references
    // Line 0 of other-page.md: "# Other Page"
    let (service, _socket, _uri_main, uri_other) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_other.clone(),
            },
            position: Position::new(0, 5), // on "Other Page"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    // No wiki links reference the "other-page" heading slug specifically,
    // so either None or empty list is acceptable.
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references for heading with no incoming links should be empty or None"
    );
}

#[tokio::test]
async fn test_references_on_plain_text_returns_none() {
    // Cursor on plain text should not return references
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references on plain text should return empty or None"
    );
}

// ---------------------------------------------------------------------------
// XML tag references
// ---------------------------------------------------------------------------

/// Helper: create workspace with XML tags for reference testing.
async fn setup_xml_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_a: Uri = "file:///workspace/a.md".parse().unwrap();
    let uri_b: Uri = "file:///workspace/b.md".parse().unwrap();

    let text_a = concat!(
        "# Doc A\n",
        "\n",
        "<agent>\n",
        "Some agent content.\n",
        "</agent>\n",
        "\n",
        "<goal>\n",
        "Win\n",
        "</goal>\n",
        "\n",
        "<agent>\n",
        "Another agent block\n",
        "</agent>\n",
    );

    let text_b = concat!(
        "# Doc B\n",
        "\n",
        "<agent>\n",
        "Agent in second doc.\n",
        "</agent>\n",
        "\n",
        "<routing>\n",
        "Some routing\n",
        "</routing>\n",
    );

    {
        let mut state = backend.state().write().await;
        let core_a = DocumentUri::new("file:///workspace/a.md").unwrap();
        let core_b = DocumentUri::new("file:///workspace/b.md").unwrap();
        state.open_document(core_a, text_a.to_string()).await;
        state.open_document(core_b, text_b.to_string()).await;
    }

    (service, socket, uri_a, uri_b)
}

#[tokio::test]
async fn test_references_for_xml_tag_same_doc() {
    // Cursor on first <agent> in a.md (line 2) -> should find all <agent> tags in same doc
    let (service, _socket, uri_a, _) = setup_xml_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_a.clone() },
            position: Position::new(2, 2), // on "<agent>"
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for XML tag with include_declaration=false should return non-declaration locations"
    );
    let locs = result.unwrap();
    // Workspace has 3 total <agent> occurrences; include_declaration=false excludes current one.
    assert_eq!(
        locs.len(),
        2,
        "should exclude declaration and keep the two non-declaration <agent> references"
    );
}

#[tokio::test]
async fn test_references_for_xml_tag_cross_doc() {
    // Cursor on <agent> in b.md (line 2) -> should find all <agent> tags across workspace
    let (service, _socket, _uri_a, uri_b) = setup_xml_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_b.clone() },
            position: Position::new(2, 2), // on "<agent>" in b.md
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for XML tag should include cross-document matches except declaration"
    );
    let locs = result.unwrap();
    // b.md has 1 <agent>, a.md has 2 <agent>; include_declaration=false excludes current b.md one.
    assert_eq!(
        locs.len(),
        2,
        "should find exactly 2 non-declaration <agent> references across workspace"
    );
    // Verify at least one reference points to a.md
    assert!(
        locs.iter()
            .any(|l| l.uri.as_str() == "file:///workspace/a.md"),
        "should include references from a.md"
    );
}

#[tokio::test]
async fn test_references_for_xml_tag_unique_no_refs() {
    // Cursor on <routing> in b.md (line 6) -> only 1 occurrence, so no other refs
    let (service, _socket, _uri_a, uri_b) = setup_xml_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_b.clone() },
            position: Position::new(6, 3), // on "<routing>" in b.md
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    // Only 1 <routing> tag in the whole workspace and include_declaration=false.
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references for a unique XML tag should exclude declaration and return empty/None"
    );
}

#[tokio::test]
async fn test_references_for_xml_tag_include_declaration_true() {
    // Cursor on unique <routing> in b.md with include_declaration=true should return itself.
    let (service, _socket, _uri_a, uri_b) = setup_xml_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_b.clone() },
            position: Position::new(6, 3), // on "<routing>" in b.md
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for a unique XML tag should include declaration when requested"
    );
    let locs = result.unwrap();
    assert_eq!(
        locs.len(),
        1,
        "should include exactly the declaration reference"
    );
    assert_eq!(locs[0].uri.as_str(), "file:///workspace/b.md");
}

// ---------------------------------------------------------------------------
// Structured document key references (marky-lkj.12)
// ---------------------------------------------------------------------------

/// Helper: create workspace with structured docs and markdown wiki-links referencing keys.
async fn setup_structured_refs_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_md: Uri = "file:///workspace/notes.md".parse().unwrap();
    let uri_json: Uri = "file:///workspace/config.json".parse().unwrap();
    let uri_yaml: Uri = "file:///workspace/settings.yaml".parse().unwrap();

    // Markdown doc with wiki-links referencing structured doc keys
    // Wiki-links use the file stem (not the full filename with extension)
    let md_text = concat!(
        "# Notes\n",
        "\n",
        "The database host is configured in [[config#database.host]].\n",
        "\n",
        "The log level is in [[settings#logging.level]].\n",
        "\n",
        "Another ref to host: [[config#database.host]].\n",
    );

    // JSON config with nested keys
    let json_text = concat!(
        "{\n",
        "  \"database\": {\n",
        "    \"host\": \"localhost\",\n",
        "    \"port\": 5432\n",
        "  }\n",
        "}\n",
    );

    // YAML settings with nested keys
    let yaml_text = concat!("logging:\n", "  level: info\n",);

    {
        let mut state = backend.state().write().await;
        let core_md = DocumentUri::new("file:///workspace/notes.md").unwrap();
        let core_json = DocumentUri::new("file:///workspace/config.json").unwrap();
        let core_yaml = DocumentUri::new("file:///workspace/settings.yaml").unwrap();
        state.open_document(core_md, md_text.to_string()).await;
        state.open_document(core_json, json_text.to_string()).await;
        state.open_document(core_yaml, yaml_text.to_string()).await;
    }

    (service, socket, uri_md, uri_json, uri_yaml)
}

#[tokio::test]
async fn test_references_structured_key_finds_markdown_wiki_links() {
    // Direction 1: Cursor on "host" key in config.json -> find markdown wiki-links referencing it
    // In the JSON: line 2: '    "host": "localhost",' — cursor on "host" key
    let (service, _socket, _uri_md, uri_json, _uri_yaml) = setup_structured_refs_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_json.clone(),
            },
            position: Position::new(2, 5), // on "host" key in JSON
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for structured key should find markdown wiki-links"
    );
    let locs = result.unwrap();
    // notes.md has two wiki-links to config.json#database.host (lines 2 and 6)
    assert_eq!(
        locs.len(),
        2,
        "should find exactly 2 wiki-link references to database.host: found {}",
        locs.len()
    );
    assert!(
        locs.iter()
            .all(|l| l.uri.as_str() == "file:///workspace/notes.md"),
        "all references should be in notes.md"
    );
}

#[tokio::test]
async fn test_references_structured_key_yaml_finds_wiki_links() {
    // Direction 1 (YAML): Cursor on "level" key in settings.yaml -> find wiki-links
    // In the YAML: line 1: '  level: info' — cursor on "level"
    let (service, _socket, _uri_md, _uri_json, uri_yaml) = setup_structured_refs_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_yaml.clone(),
            },
            position: Position::new(1, 3), // on "level" key in YAML
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references for YAML key should find markdown wiki-links"
    );
    let locs = result.unwrap();
    assert_eq!(
        locs.len(),
        1,
        "should find exactly 1 wiki-link reference to logging.level: found {}",
        locs.len()
    );
    assert_eq!(locs[0].uri.as_str(), "file:///workspace/notes.md");
}

#[tokio::test]
async fn test_references_structured_key_no_refs() {
    // Cursor on "port" key in config.json -> no wiki-links reference it
    // In the JSON: line 3: '    "port": 5432' — cursor on "port"
    let (service, _socket, _uri_md, uri_json, _uri_yaml) = setup_structured_refs_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_json.clone(),
            },
            position: Position::new(3, 5), // on "port" key in JSON
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    let is_empty = result.as_ref().is_none_or(|v| v.is_empty());
    assert!(
        is_empty,
        "references for structured key with no incoming wiki-links should be empty/None"
    );
}

#[tokio::test]
async fn test_references_structured_key_include_declaration() {
    // Cursor on "host" key with include_declaration=true -> should include the key itself
    let (service, _socket, _uri_md, uri_json, _uri_yaml) = setup_structured_refs_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_json.clone(),
            },
            position: Position::new(2, 5), // on "host" key in JSON
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references with include_declaration=true should include declaration + wiki-links"
    );
    let locs = result.unwrap();
    // 2 wiki-links + 1 declaration = 3
    assert_eq!(
        locs.len(),
        3,
        "should find 2 wiki-link refs + 1 declaration: found {}",
        locs.len()
    );
    // Verify the declaration is in config.json
    assert!(
        locs.iter()
            .any(|l| l.uri.as_str() == "file:///workspace/config.json"),
        "should include the key declaration in config.json"
    );
}

#[tokio::test]
async fn test_references_wiki_link_to_structured_key_finds_definition() {
    // Direction 2: Cursor on wiki-link [[config#database.host]] in notes.md
    // -> should find the key definition location + other wiki-links to same key
    // Line 2: "The database host is configured in [[config#database.host]]."
    // The wiki-link starts at column 35
    let (service, _socket, uri_md, _uri_json, _uri_yaml) = setup_structured_refs_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_md.clone(),
            },
            position: Position::new(2, 40), // inside [[config#database.host]]
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    assert!(
        result.is_some(),
        "references from wiki-link to structured key should find related locations"
    );
    let locs = result.unwrap();
    // Should find: the key definition in config.json + the other wiki-link on line 6
    // (exclude the current wiki-link since include_declaration=false)
    assert!(
        !locs.is_empty(),
        "should find at least 1 reference (key definition or other wiki-link)"
    );
    // The key definition should be in config.json
    assert!(
        locs.iter()
            .any(|l| l.uri.as_str() == "file:///workspace/config.json"),
        "should include the key definition location in config.json"
    );
}
