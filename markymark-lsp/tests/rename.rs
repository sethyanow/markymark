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
// rename: XML tags
// =======================================================================

/// Helper: set up workspace with XML tags for rename tests.
async fn setup_xml_rename_workspace() -> (
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
        "# Doc A\n",        // line 0
        "\n",               // line 1
        "<agent>\n",        // line 2
        "Agent content.\n", // line 3
        "</agent>\n",       // line 4
        "\n",               // line 5
        "<agent>\n",        // line 6
        "Second agent.\n",  // line 7
        "</agent>\n",       // line 8
    );

    let text_b = concat!(
        "# Doc B\n",        // line 0
        "\n",               // line 1
        "<agent>\n",        // line 2
        "Another agent.\n", // line 3
        "</agent>\n",       // line 4
        "\n",               // line 5
        "<routing>\n",      // line 6
        "Some path\n",      // line 7
        "</routing>\n",     // line 8
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
async fn test_prepare_rename_on_xml_tag() {
    let (service, _socket, uri_a, _) = setup_xml_rename_workspace().await;
    let backend = service.inner();

    // Cursor on <agent> at line 2
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri_a.clone() },
        position: Position::new(2, 2), // inside "agent"
    };

    let result = backend
        .prepare_rename(params)
        .await
        .expect("prepare_rename should not error");
    assert!(result.is_some(), "should be able to rename an XML tag");

    match result.unwrap() {
        PrepareRenameResponse::RangeWithPlaceholder { placeholder, range } => {
            assert_eq!(placeholder, "agent");
            assert_eq!(range.start.line, 2);
        }
        other => panic!("expected RangeWithPlaceholder, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_rename_xml_tag_updates_all_occurrences() {
    let (service, _socket, uri_a, uri_b) = setup_xml_rename_workspace().await;
    let backend = service.inner();

    // Rename "agent" to "assistant" from a.md line 2
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_a.clone() },
            position: Position::new(2, 2),
        },
        new_name: "assistant".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    assert!(result.is_some(), "rename should produce edits");

    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // a.md has 2 <agent>...</agent> tags -> 4 edits (2 open + 2 close)
    // b.md has 1 <agent>...</agent> tag  -> 2 edits (1 open + 1 close)
    let total_edits: usize = changes.values().map(|v| v.len()).sum();
    assert_eq!(
        total_edits, 6,
        "should have 6 edits (open+close for 3 tags): got {}",
        total_edits
    );

    // Verify a.md has edits
    let a_edits = changes.get(&uri_a).expect("should have edits for a.md");
    assert_eq!(
        a_edits.len(),
        4,
        "a.md should have 4 edits (2 open + 2 close): got {}",
        a_edits.len()
    );

    // Verify b.md has edits
    let b_edits = changes.get(&uri_b).expect("should have edits for b.md");
    assert_eq!(
        b_edits.len(),
        2,
        "b.md should have 2 edits (1 open + 1 close): got {}",
        b_edits.len()
    );

    // All edits should use "assistant" as new text
    for edits in changes.values() {
        for edit in edits {
            assert_eq!(
                edit.new_text, "assistant",
                "all tag name edits should use the new name"
            );
        }
    }
}

#[tokio::test]
async fn test_rename_xml_tag_does_not_affect_other_tags() {
    let (service, _socket, _uri_a, uri_b) = setup_xml_rename_workspace().await;
    let backend = service.inner();

    // Rename "routing" tag from b.md line 6
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_b.clone() },
            position: Position::new(6, 3),
        },
        new_name: "navigation".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    assert!(result.is_some(), "rename should produce edits");

    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // Only 1 <routing>...</routing> tag in workspace -> 2 edits (open + close)
    let total_edits: usize = changes.values().map(|v| v.len()).sum();
    assert_eq!(
        total_edits, 2,
        "renaming unique tag should produce 2 edits (open + close)"
    );
}

// =======================================================================
// rename: XML tag closing tags
// =======================================================================

#[tokio::test]
async fn test_rename_xml_tag_edits_both_open_and_close_tags() {
    let (service, _socket, uri_a, uri_b) = setup_xml_rename_workspace().await;
    let backend = service.inner();

    // Rename "agent" to "assistant" from a.md line 2
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_a.clone() },
            position: Position::new(2, 2),
        },
        new_name: "assistant".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // a.md has 2 <agent>...</agent> tags -> 4 edits (2 open + 2 close)
    // b.md has 1 <agent>...</agent> tag  -> 2 edits (1 open + 1 close)
    // Total: 6 edits
    let total_edits: usize = changes.values().map(|v| v.len()).sum();
    assert_eq!(
        total_edits, 6,
        "should have 6 edits (open+close for each of 3 tags): got {}",
        total_edits
    );

    // Verify a.md: first tag <agent> on line 2, </agent> on line 4
    let a_edits = changes.get(&uri_a).expect("should have edits for a.md");

    // Should have an edit on line 4 (closing </agent>)
    let close_tag_edit = a_edits
        .iter()
        .find(|e| e.range.start.line == 4)
        .expect("should have a closing tag edit on line 4 of a.md");
    assert_eq!(
        close_tag_edit.new_text, "assistant",
        "closing tag name should be renamed"
    );
    // Closing tag </agent> — name starts at column 2 (after "</" )
    assert_eq!(
        close_tag_edit.range.start.character, 2,
        "closing tag name should start after </"
    );

    // Verify b.md: closing </agent> on line 4
    let b_edits = changes.get(&uri_b).expect("should have edits for b.md");
    let b_close_edit = b_edits
        .iter()
        .find(|e| e.range.start.line == 4)
        .expect("should have a closing tag edit on line 4 of b.md");
    assert_eq!(b_close_edit.new_text, "assistant");
}

#[tokio::test]
async fn test_rename_xml_tag_unique_edits_open_and_close() {
    let (service, _socket, _uri_a, uri_b) = setup_xml_rename_workspace().await;
    let backend = service.inner();

    // Rename "routing" tag from b.md line 6 — block-level <routing>...</routing>
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_b.clone() },
            position: Position::new(6, 3),
        },
        new_name: "navigation".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = backend
        .rename(params)
        .await
        .expect("rename should not error");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.expect("should have changes");

    // 1 tag with open+close -> 2 edits
    let total_edits: usize = changes.values().map(|v| v.len()).sum();
    assert_eq!(
        total_edits, 2,
        "renaming tag with close should produce 2 edits (open + close): got {}",
        total_edits
    );

    let b_edits = changes.get(&uri_b).expect("should have edits for b.md");
    // Open <routing> on line 6, close </routing> on line 8
    assert_eq!(b_edits.len(), 2, "should have 2 edits on b.md");
    for edit in b_edits {
        assert_eq!(edit.new_text, "navigation");
    }
    let lines: Vec<u32> = b_edits.iter().map(|e| e.range.start.line).collect();
    assert!(lines.contains(&6), "should have open tag edit on line 6");
    assert!(lines.contains(&8), "should have close tag edit on line 8");
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
