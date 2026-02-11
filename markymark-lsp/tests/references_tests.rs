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
        state.open_document(core_main, main_text.to_string());
        state.open_document(core_other, other_text.to_string());
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
        "<goal>Win</goal>\n",
        "\n",
        "<agent>Another agent block</agent>\n",
    );

    let text_b = concat!(
        "# Doc B\n",
        "\n",
        "<agent>\n",
        "Agent in second doc.\n",
        "</agent>\n",
        "\n",
        "<routing>Some routing</routing>\n",
    );

    {
        let mut state = backend.state().write().await;
        let core_a = DocumentUri::new("file:///workspace/a.md").unwrap();
        let core_b = DocumentUri::new("file:///workspace/b.md").unwrap();
        state.open_document(core_a, text_a.to_string());
        state.open_document(core_b, text_b.to_string());
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
            text_document: TextDocumentIdentifier {
                uri: uri_a.clone(),
            },
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
        "references for XML tag with multiple occurrences should return locations"
    );
    let locs = result.unwrap();
    // a.md has <agent> on line 2 and line 8 -> at least 2 locations
    assert!(
        locs.len() >= 2,
        "should find at least 2 <agent> references in a.md: found {}",
        locs.len()
    );
    // All locations should be in a.md (and b.md has one too, so actually >= 3 total)
    assert!(
        locs.len() >= 3,
        "should find at least 3 <agent> references across workspace: found {}",
        locs.len()
    );
}

#[tokio::test]
async fn test_references_for_xml_tag_cross_doc() {
    // Cursor on <agent> in b.md (line 2) -> should find all <agent> tags across workspace
    let (service, _socket, _uri_a, uri_b) = setup_xml_workspace().await;
    let backend = service.inner();

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_b.clone(),
            },
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
        "references for XML tag should include cross-document matches"
    );
    let locs = result.unwrap();
    // b.md has 1 <agent>, a.md has 2 <agent> -> at least 3 total
    assert!(
        locs.len() >= 3,
        "should find at least 3 <agent> references across workspace: found {}",
        locs.len()
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
            text_document: TextDocumentIdentifier {
                uri: uri_b.clone(),
            },
            position: Position::new(6, 3), // on "<routing>" in b.md
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = backend.references(params).await.unwrap();
    // Only 1 <routing> tag in the whole workspace -> return that single location
    // (find-references typically includes all occurrences, including the one under cursor)
    assert!(
        result.is_some(),
        "references for a unique XML tag should return at least itself"
    );
    let locs = result.unwrap();
    assert_eq!(locs.len(), 1, "should find exactly 1 <routing> reference");
}
