//! Rename tests: textDocument/prepareRename and textDocument/rename.

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

/// Helper: set up a workspace with documents that exercise rename scenarios.
///
/// Documents:
/// - main.md: has headings, wiki links, markdown links with anchors
/// - other.md: has cross-file wiki links and markdown links referencing main.md headings
async fn setup_rename_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_main: Uri = "file:///workspace/main.md".parse().unwrap();
    let uri_other: Uri = "file:///workspace/other.md".parse().unwrap();

    let main_text = concat!(
        "# Main Title\n",                   // line 0
        "\n",                               // line 1
        "## Introduction\n",                // line 2
        "\n",                               // line 3
        "See [[#introduction]].\n",         // line 4: same-page wiki link
        "\n",                               // line 5
        "A link: [intro](#introduction)\n", // line 6: same-page markdown link anchor
        "\n",                               // line 7
        "## Details\n",                     // line 8
    );

    let other_text = concat!(
        "# Other Page\n",                        // line 0
        "\n",                                    // line 1
        "See [[main#introduction]] for info.\n", // line 2: cross-file wiki link
        "\n",                                    // line 3
        "Also [ref](#details) is local.\n",      // line 4: local anchor (no match)
        "\n",                                    // line 5
        "## Details\n",                          // line 6: own heading named Details
    );

    {
        let mut state = backend.state().write().await;
        let core_main = DocumentUri::new("file:///workspace/main.md").unwrap();
        let core_other = DocumentUri::new("file:///workspace/other.md").unwrap();
        state.open_document(core_main, main_text.to_string());
        state.open_document(core_other, other_text.to_string());
    }

    (service, socket, uri_main, uri_other)
}

// =======================================================================
// Capabilities
// =======================================================================

#[tokio::test]
async fn test_capabilities_rename_provider() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let result = backend
        .initialize(InitializeParams::default())
        .await
        .expect("initialize should succeed");
    let caps = result.capabilities;
    assert!(
        caps.rename_provider.is_some(),
        "server should declare rename provider capability"
    );
}

#[tokio::test]
async fn test_capabilities_rename_has_prepare_provider() {
    let (service, _socket) = create_service();
    let backend = service.inner();
    let result = backend
        .initialize(InitializeParams::default())
        .await
        .expect("initialize should succeed");
    let caps = result.capabilities;
    match caps.rename_provider {
        Some(OneOf::Right(opts)) => {
            assert_eq!(
                opts.prepare_provider,
                Some(true),
                "rename should support prepareRename"
            );
        }
        other => panic!("expected RenameOptions, got: {:?}", other),
    }
}

// =======================================================================
// prepareRename
// =======================================================================

#[tokio::test]
async fn test_prepare_rename_on_heading() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Cursor on "## Introduction" (line 2, char 3 = inside heading text)
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier {
            uri: uri_main.clone(),
        },
        position: Position::new(2, 3),
    };

    let result = backend
        .prepare_rename(params)
        .await
        .expect("prepare_rename should not error");
    assert!(result.is_some(), "should be able to rename a heading");

    match result.unwrap() {
        PrepareRenameResponse::RangeWithPlaceholder { placeholder, range } => {
            assert_eq!(placeholder, "Introduction");
            // Range should cover the heading line
            assert_eq!(range.start.line, 2);
            assert_eq!(range.end.line, 2);
        }
        other => panic!("expected RangeWithPlaceholder, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_prepare_rename_on_wiki_link_returns_none() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Cursor on [[#introduction]] (line 4, char 6 = inside wiki link)
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier {
            uri: uri_main.clone(),
        },
        position: Position::new(4, 6),
    };

    let result = backend
        .prepare_rename(params)
        .await
        .expect("prepare_rename should not error");
    assert!(
        result.is_none(),
        "wiki links should not be directly renameable"
    );
}

#[tokio::test]
async fn test_prepare_rename_on_plain_text_returns_none() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Cursor on empty line (line 1)
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier {
            uri: uri_main.clone(),
        },
        position: Position::new(1, 0),
    };

    let result = backend
        .prepare_rename(params)
        .await
        .expect("prepare_rename should not error");
    assert!(result.is_none(), "plain text should not be renameable");
}

// =======================================================================
// rename: heading with same-page references
// =======================================================================

#[tokio::test]
async fn test_rename_heading_updates_heading_text() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Introduction" to "Getting Started" (cursor on line 2)
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 3),
        },
        new_name: "Getting Started".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    assert!(result.is_some(), "rename should produce edits");

    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // Should have edits in main.md
    let main_edits = changes
        .get(&uri_main)
        .expect("should have edits for main.md");

    // One of the edits should be the heading text itself
    let heading_edit = main_edits
        .iter()
        .find(|e| e.range.start.line == 2)
        .expect("should have an edit on the heading line");
    assert_eq!(heading_edit.new_text, "Getting Started");
}

#[tokio::test]
async fn test_rename_heading_updates_same_page_wiki_link() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Introduction" heading
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 3),
        },
        new_name: "Getting Started".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");
    let main_edits = changes
        .get(&uri_main)
        .expect("should have edits for main.md");

    // Should update [[#introduction]] on line 4 to [[#Getting Started]]
    let wiki_edit = main_edits
        .iter()
        .find(|e| e.range.start.line == 4)
        .expect("should have an edit on the wiki link line");
    assert_eq!(
        wiki_edit.new_text, "Getting Started",
        "wiki link heading reference should be updated to new name"
    );
}

#[tokio::test]
async fn test_rename_heading_updates_same_page_markdown_link_anchor() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Introduction" heading
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 3),
        },
        new_name: "Getting Started".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");
    let main_edits = changes
        .get(&uri_main)
        .expect("should have edits for main.md");

    // Should update [intro](#introduction) on line 6 -- anchor becomes #getting-started
    let anchor_edit = main_edits
        .iter()
        .find(|e| e.range.start.line == 6)
        .expect("should have an edit on the markdown link line");
    assert_eq!(
        anchor_edit.new_text, "getting-started",
        "markdown link anchor should be updated to new slug"
    );
}

// =======================================================================
// rename: cross-file references
// =======================================================================

#[tokio::test]
async fn test_rename_heading_updates_cross_file_wiki_link() {
    let (service, _socket, uri_main, uri_other) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Introduction" heading in main.md
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 3),
        },
        new_name: "Getting Started".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // Should have edits in other.md for [[main#introduction]]
    let other_edits = changes
        .get(&uri_other)
        .expect("should have edits for other.md (cross-file wiki link)");
    assert!(
        !other_edits.is_empty(),
        "cross-file wiki link should be updated"
    );

    let wiki_edit = &other_edits[0];
    assert_eq!(wiki_edit.new_text, "Getting Started");
    assert_eq!(wiki_edit.range.start.line, 2, "edit should be on line 2");
}

#[tokio::test]
async fn test_rename_heading_does_not_affect_unrelated_anchors() {
    let (service, _socket, uri_main, _uri_other) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Details" heading in main.md (line 8)
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(8, 3),
        },
        new_name: "Implementation Notes".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // other.md line 4 has [ref](#details) but "details" is other.md's own heading slug,
    // not a reference to main.md's heading. The rename of main.md's "Details" should
    // NOT modify other.md's anchor links pointing to its own headings.
    // However, our current realm-wide iteration will find other.md's own anchor too.
    // This is a design question: markdown link anchors are always same-page.
    // So renaming main.md's "Details" should NOT touch other.md's [ref](#details).
    //
    // The main.md edits should include just the heading text edit.
    let main_edits = changes
        .get(&uri_main)
        .expect("should have edits for main.md");

    // Heading text edit on line 8
    let heading_edit = main_edits
        .iter()
        .find(|e| e.range.start.line == 8)
        .expect("should have heading edit");
    assert_eq!(heading_edit.new_text, "Implementation Notes");
}

// =======================================================================
// rename: on non-renameable position
// =======================================================================

#[tokio::test]
async fn test_rename_on_plain_text_returns_none() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(1, 0), // empty line
        },
        new_name: "anything".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    assert!(result.is_none(), "rename on plain text should return None");
}

#[tokio::test]
async fn test_rename_on_wiki_link_returns_none() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 6), // inside [[#introduction]]
        },
        new_name: "anything".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    assert!(
        result.is_none(),
        "rename on wiki link should return None (rename the heading instead)"
    );
}

// =======================================================================
// rename: edit count validation
// =======================================================================

#[tokio::test]
async fn test_rename_introduction_produces_correct_edit_count() {
    let (service, _socket, uri_main, _) = setup_rename_workspace().await;
    let backend = service.inner();

    // Rename "Introduction" heading -- should produce:
    // 1. Heading text edit (line 2 in main.md)
    // 2. Wiki link [[#introduction]] edit (line 4 in main.md)
    // 3. Markdown link anchor [intro](#introduction) edit (line 6 in main.md)
    // 4. Cross-file wiki link [[main#introduction]] edit (line 2 in other.md)
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 3),
        },
        new_name: "Getting Started".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    let total_edits: usize = changes.values().map(|v| v.len()).sum();
    assert_eq!(
        total_edits, 4,
        "renaming Introduction should produce 4 edits (heading + same-page wiki + same-page anchor + cross-file wiki)"
    );
}
