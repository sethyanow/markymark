use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use markymark_mcp::RuntimeEngine;

use super::{compare_ranges, TempWorkspace};

/// Helper to flatten WorkspaceEdit into a vec of (uri, range, new_text) sorted deterministically.
fn flatten_workspace_edit(result: CoreOperationResult) -> Vec<(DocumentUri, Range, String)> {
    match result {
        CoreOperationResult::WorkspaceEdit(edits) => {
            let mut flat: Vec<(DocumentUri, Range, String)> = edits
                .into_iter()
                .flat_map(|(uri, changes)| {
                    changes
                        .into_iter()
                        .map(move |(range, text)| (uri.clone(), range, text))
                })
                .collect();
            flat.sort_by(|(uri_a, range_a, _), (uri_b, range_b, _)| {
                uri_a
                    .as_str()
                    .cmp(uri_b.as_str())
                    .then_with(|| compare_ranges(*range_a, *range_b))
            });
            flat
        }
        other => panic!("expected WorkspaceEdit, got: {other:?}"),
    }
}

#[tokio::test]
async fn rename_heading_edits_heading_text_and_wiki_link_and_markdown_anchor() {
    let ws = TempWorkspace::new("rename-heading");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Title\n\n## Setup\n\nSee [[#setup]] here.\n\nAlso [link](#setup) works.\n",
    )
    .expect("a.md should be created");

    let b = ws.root().join("b.md");
    fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 3), Position::new(2, 3)),
            new_name: "Installation".to_string(),
            realm: None,
        })
        .await;

    let edits = flatten_workspace_edit(result);
    assert!(
        edits.len() >= 3,
        "expected at least 3 rename edits, got {}",
        edits.len()
    );

    let a_uri = DocumentUri::from_file_path(&a);
    let heading_edits: Vec<_> = edits
        .iter()
        .filter(|(uri, _, text)| *uri == a_uri && text == "Installation")
        .collect();
    assert!(
        !heading_edits.is_empty(),
        "should have at least one edit replacing heading text with 'Installation'"
    );
}

#[tokio::test]
async fn rename_xml_tag_edits_open_and_close_tags_across_documents() {
    let ws = TempWorkspace::new("rename-xml");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Doc A\n\n<agent>content</agent>\n").expect("a.md should be created");

    let b = ws.root().join("b.md");
    fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
            new_name: "tool".to_string(),
            realm: None,
        })
        .await;

    let edits = flatten_workspace_edit(result);
    assert_eq!(
        edits.len(),
        4,
        "expected 4 XML rename edits (2 per tag), got {}",
        edits.len()
    );

    for (_, _, new_text) in &edits {
        assert_eq!(new_text, "tool", "all edits should rename to 'tool'");
    }
}

#[tokio::test]
async fn rename_self_closing_xml_tag_edits_only_open_tag() {
    let ws = TempWorkspace::new("rename-xml-self-close");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Doc\n\n<br/>\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
            new_name: "hr".to_string(),
            realm: None,
        })
        .await;

    let edits = flatten_workspace_edit(result);
    assert_eq!(
        edits.len(),
        1,
        "expected 1 edit for self-closing XML tag, got {}",
        edits.len()
    );
    assert_eq!(edits[0].2, "hr");
}

#[tokio::test]
async fn rename_returns_error_for_unknown_document() {
    let ws = TempWorkspace::new("rename-unknown");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let unknown = ws.root().join("nonexistent.md");
    let result = engine
        .execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&unknown),
            position: Range::new(Position::new(0, 2), Position::new(0, 2)),
            new_name: "NewName".to_string(),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown document, got: {other:?}"),
    }
}

#[tokio::test]
async fn rename_returns_error_for_position_without_renameable_symbol() {
    let ws = TempWorkspace::new("rename-nosymbol");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 2), Position::new(2, 2)),
            new_name: "Whatever".to_string(),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for no-symbol position, got: {other:?}"),
    }
}
