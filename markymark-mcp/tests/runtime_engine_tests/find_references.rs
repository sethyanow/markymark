use std::cmp::Ordering;
use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use markymark_mcp::RuntimeEngine;

use super::{compare_ranges, TempWorkspace};

#[tokio::test]
async fn find_references_returns_wiki_link_refs_to_heading() {
    let ws = TempWorkspace::new("find-refs-heading");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Title\n\n## Setup\n\nSee [[#setup]] for info.\n")
        .expect("a.md should be created");
    let b = ws.root().join("b.md");
    fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 3), Position::new(2, 3)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                locations.len() >= 2,
                "expected at least 2 references, got {}",
                locations.len()
            );
            for window in locations.windows(2) {
                let (uri_a, range_a) = &window[0];
                let (uri_b, range_b) = &window[1];
                let ord = uri_a
                    .as_str()
                    .cmp(uri_b.as_str())
                    .then_with(|| compare_ranges(*range_a, *range_b));
                assert!(
                    ord != Ordering::Greater,
                    "locations should be sorted, but {uri_a:?} > {uri_b:?}"
                );
            }
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_returns_markdown_link_refs_to_heading() {
    let ws = TempWorkspace::new("find-refs-mdlink");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Title\n\n## My Section\n\nSee [link](#my-section) here.\n",
    )
    .expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 4), Position::new(2, 4)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                !locations.is_empty(),
                "expected at least 1 markdown link reference"
            );
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_returns_xml_tag_refs_across_documents() {
    let ws = TempWorkspace::new("find-refs-xml");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Doc A\n\n<agent>content</agent>\n\n<agent>more</agent>\n",
    )
    .expect("a.md should be created");
    let b = ws.root().join("b.md");
    fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert_eq!(
                locations.len(),
                3,
                "expected 3 XML tag references, got {}",
                locations.len()
            );
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_returns_error_for_unknown_document() {
    let ws = TempWorkspace::new("find-refs-unknown");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let unknown = ws.root().join("nonexistent.md");
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&unknown),
            position: Range::new(Position::new(0, 2), Position::new(0, 2)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown document, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_returns_error_for_position_without_symbol() {
    let ws = TempWorkspace::new("find-refs-nosymbol");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 2), Position::new(2, 2)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Error(_) => { /* expected - no symbol at position */ }
        other => panic!("expected error for no-symbol position, got: {other:?}"),
    }
}

// --- find-references: block_ref support (marky-jrw) ---

const BLOCK_UUID_A: &str = "550e8400-e29b-41d4-a716-446655440000";
const BLOCK_UUID_B: &str = "7f6c1b2a-3d4e-5f60-a7b8-c9d0e1f20304";

#[tokio::test]
async fn find_references_for_block_ref_returns_all_referencing_docs() {
    let ws = TempWorkspace::new("find-refs-blockref");
    let a = ws.root().join("a.md");
    let b = ws.root().join("b.md");
    let c = ws.root().join("c.md");

    // Doc A: contains the target block ref
    fs::write(&a, format!("(({BLOCK_UUID_A})) is here\n")).expect("a.md should be created");
    // Doc B: contains the same block ref plus another
    fs::write(
        &b,
        format!("Some text\n\n(({BLOCK_UUID_A})) and (({BLOCK_UUID_B}))\n"),
    )
    .expect("b.md should be created");
    // Doc C: no block refs
    fs::write(&c, "# No refs\n").expect("c.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    // Position cursor inside ((uuid)) in Doc A — line 0, char 3 is inside the UUID text
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(0, 3), Position::new(0, 3)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                locations.len() >= 2,
                "expected at least 2 block ref locations (a.md and b.md), got {}",
                locations.len()
            );
            let c_uri = DocumentUri::from_file_path(&c);
            assert!(
                !locations.iter().any(|(uri, _)| *uri == c_uri),
                "Doc C (no block refs) should not appear in results"
            );
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_block_ref_results_sorted_by_uri_then_range() {
    let ws = TempWorkspace::new("find-refs-blockref-sorted");

    // Create files with names that sort alphabetically: a.md < b.md < c.md
    fs::write(ws.root().join("c.md"), format!("(({BLOCK_UUID_A})) in c\n"))
        .expect("c.md should be created");
    fs::write(ws.root().join("a.md"), format!("(({BLOCK_UUID_A})) in a\n"))
        .expect("a.md should be created");
    fs::write(ws.root().join("b.md"), format!("(({BLOCK_UUID_A})) in b\n"))
        .expect("b.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&ws.root().join("a.md")),
            position: Range::new(Position::new(0, 3), Position::new(0, 3)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert_eq!(
                locations.len(),
                3,
                "expected 3 block ref locations, got {}",
                locations.len()
            );
            for window in locations.windows(2) {
                let (uri_a, range_a) = &window[0];
                let (uri_b, range_b) = &window[1];
                let ord = uri_a
                    .as_str()
                    .cmp(uri_b.as_str())
                    .then_with(|| compare_ranges(*range_a, *range_b));
                assert!(
                    ord != Ordering::Greater,
                    "locations should be sorted by uri then range, but {uri_a:?} > {uri_b:?}"
                );
            }
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}

#[tokio::test]
async fn find_references_for_block_id_returns_block_ref_locations() {
    let ws = TempWorkspace::new("find-refs-block-id-inverse");

    // Doc A: defines a block with ^uuid
    let a = ws.root().join("a.md");
    fs::write(&a, format!("some content ^{BLOCK_UUID_A}\n")).expect("a.md should be created");
    // Doc B: references that block with ((uuid))
    let b = ws.root().join("b.md");
    fs::write(&b, format!("(({BLOCK_UUID_A})) is referenced\n")).expect("b.md should be created");
    // Doc C: no refs
    let c = ws.root().join("c.md");
    fs::write(&c, "# No refs\n").expect("c.md should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    // Cursor inside ^uuid in Doc A: "some content ^550e8400..."
    // ^  is at position 13, UUID starts at 14; position 16 is inside the UUID
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(0, 16), Position::new(0, 16)),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                !locations.is_empty(),
                "expected at least 1 location pointing to ((uuid)) in Doc B"
            );
            let b_uri = DocumentUri::from_file_path(&b);
            assert!(
                locations.iter().any(|(uri, _)| *uri == b_uri),
                "Doc B should appear in results (contains ((uuid)))"
            );
            let c_uri = DocumentUri::from_file_path(&c);
            assert!(
                !locations.iter().any(|(uri, _)| *uri == c_uri),
                "Doc C should not appear in results"
            );
        }
        other => panic!("expected Locations result, got: {other:?}"),
    }
}
