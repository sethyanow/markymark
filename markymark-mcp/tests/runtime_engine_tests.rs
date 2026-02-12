//! Integration tests for `RuntimeEngine` (the MCP core engine implementation).
//!
//! Extracted from `src/runtime_engine.rs` during the z6r refactor.

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use markymark_mcp::RuntimeEngine;

/// Compare two ranges for deterministic sorting (test-local copy).
fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "markymark-mcp-runtime-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary workspace directory should be created");
        Self { root }
    }

    fn root(&self) -> PathBuf {
        self.root.clone()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn rejects_empty_workspace_roots() {
    let err = match RuntimeEngine::from_workspace_roots(Vec::new()) {
        Ok(_) => panic!("empty workspace roots should fail"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .contains("at least one workspace root is required"));
}

#[test]
fn rejects_missing_workspace_root() {
    let missing = std::env::temp_dir().join("markymark-missing-workspace");
    if missing.exists() {
        fs::remove_dir_all(&missing).expect("stale missing-workspace path should be removable");
    }

    let err = match RuntimeEngine::from_workspace_roots(vec![missing.clone()]) {
        Ok(_) => panic!("missing workspace root should fail"),
        Err(err) => err,
    };
    assert!(err.to_string().contains(&format!(
        "workspace root does not exist: {}",
        missing.display()
    )));
}

#[test]
fn rejects_workspace_root_file_path() {
    let ws = TempWorkspace::new("root-file");
    let file_path = ws.root().join("not-a-directory.md");
    fs::write(&file_path, "# Heading").expect("test file should be created");

    let err = match RuntimeEngine::from_workspace_roots(vec![file_path.clone()]) {
        Ok(_) => panic!("workspace root file should fail"),
        Err(err) => err,
    };
    assert!(err.to_string().contains(&format!(
        "workspace root is not a directory: {}",
        file_path.display()
    )));
}

#[test]
fn indexes_markdown_and_returns_deterministic_symbols() {
    let ws = TempWorkspace::new("indexed");
    let first = ws.root().join("a.md");
    let second = ws.root().join("b.md");
    fs::write(&first, "# Zebra\n## Alpha\n").expect("first markdown should be created");
    fs::write(&second, "# Beta\n").expect("second markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let outline = engine.execute(CoreOperation::GetOutline {
        uri: DocumentUri::from_file_path(&first),
    });
    match outline {
        CoreOperationResult::Outline(headings) => {
            assert_eq!(headings, vec!["Zebra".to_string(), "Alpha".to_string()]);
        }
        other => panic!("expected outline result, got: {other:?}"),
    }

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "a".to_string(),
    });
    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(
                names,
                vec!["Alpha".to_string(), "Beta".to_string(), "Zebra".to_string()]
            );
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[test]
fn find_references_returns_wiki_link_refs_to_heading() {
    let ws = TempWorkspace::new("find-refs-heading");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Title\n\n## Setup\n\nSee [[#setup]] for info.\n")
        .expect("a.md should be created");
    let b = ws.root().join("b.md");
    fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 3), Position::new(2, 3)),
    });

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

#[test]
fn find_references_returns_markdown_link_refs_to_heading() {
    let ws = TempWorkspace::new("find-refs-mdlink");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Title\n\n## My Section\n\nSee [link](#my-section) here.\n",
    )
    .expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 4), Position::new(2, 4)),
    });

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

#[test]
fn find_references_returns_xml_tag_refs_across_documents() {
    let ws = TempWorkspace::new("find-refs-xml");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Doc A\n\n<agent>content</agent>\n\n<agent>more</agent>\n",
    )
    .expect("a.md should be created");
    let b = ws.root().join("b.md");
    fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 1), Position::new(2, 1)),
    });

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

#[test]
fn find_references_returns_error_for_unknown_document() {
    let ws = TempWorkspace::new("find-refs-unknown");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let unknown = ws.root().join("nonexistent.md");
    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&unknown),
        position: Range::new(Position::new(0, 2), Position::new(0, 2)),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown document, got: {other:?}"),
    }
}

#[test]
fn find_references_returns_error_for_position_without_symbol() {
    let ws = TempWorkspace::new("find-refs-nosymbol");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 2), Position::new(2, 2)),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected - no symbol at position */ }
        other => panic!("expected error for no-symbol position, got: {other:?}"),
    }
}

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

#[test]
fn rename_heading_edits_heading_text_and_wiki_link_and_markdown_anchor() {
    let ws = TempWorkspace::new("rename-heading");
    let a = ws.root().join("a.md");
    fs::write(
        &a,
        "# Title\n\n## Setup\n\nSee [[#setup]] here.\n\nAlso [link](#setup) works.\n",
    )
    .expect("a.md should be created");

    let b = ws.root().join("b.md");
    fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::Rename {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 3), Position::new(2, 3)),
        new_name: "Installation".to_string(),
    });

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

#[test]
fn rename_xml_tag_edits_open_and_close_tags_across_documents() {
    let ws = TempWorkspace::new("rename-xml");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Doc A\n\n<agent>content</agent>\n").expect("a.md should be created");

    let b = ws.root().join("b.md");
    fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::Rename {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 1), Position::new(2, 1)),
        new_name: "tool".to_string(),
    });

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

#[test]
fn rename_self_closing_xml_tag_edits_only_open_tag() {
    let ws = TempWorkspace::new("rename-xml-self-close");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Doc\n\n<br/>\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::Rename {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 1), Position::new(2, 1)),
        new_name: "hr".to_string(),
    });

    let edits = flatten_workspace_edit(result);
    assert_eq!(
        edits.len(),
        1,
        "expected 1 edit for self-closing XML tag, got {}",
        edits.len()
    );
    assert_eq!(edits[0].2, "hr");
}

#[test]
fn rename_returns_error_for_unknown_document() {
    let ws = TempWorkspace::new("rename-unknown");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let unknown = ws.root().join("nonexistent.md");
    let result = engine.execute(CoreOperation::Rename {
        uri: DocumentUri::from_file_path(&unknown),
        position: Range::new(Position::new(0, 2), Position::new(0, 2)),
        new_name: "NewName".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown document, got: {other:?}"),
    }
}

#[test]
fn rename_returns_error_for_position_without_renameable_symbol() {
    let ws = TempWorkspace::new("rename-nosymbol");
    let a = ws.root().join("a.md");
    fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::Rename {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(2, 2), Position::new(2, 2)),
        new_name: "Whatever".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for no-symbol position, got: {other:?}"),
    }
}

// === Realm Management Tests ===

#[test]
fn create_realm_returns_realm_info() {
    let ws = TempWorkspace::new("create-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::CreateRealm {
        name: "test-realm".to_string(),
    });

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "test-realm");
            assert_eq!(root_count, 0);
            assert_eq!(document_count, 0);
        }
        other => panic!("expected RealmInfo, got: {other:?}"),
    }
}

#[test]
fn create_realm_rejects_duplicate_name() {
    let ws = TempWorkspace::new("create-dup-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "my-realm".to_string(),
    });

    let result = engine.execute(CoreOperation::CreateRealm {
        name: "my-realm".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for duplicate realm, got: {other:?}"),
    }
}

#[test]
fn create_realm_rejects_empty_name() {
    let ws = TempWorkspace::new("create-empty-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::CreateRealm {
        name: "".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for empty realm name, got: {other:?}"),
    }
}

#[test]
fn destroy_realm_removes_realm() {
    let ws = TempWorkspace::new("destroy-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "temp-realm".to_string(),
    });
    let result = engine.execute(CoreOperation::DestroyRealm {
        name: "temp-realm".to_string(),
    });

    match result {
        CoreOperationResult::Ok => { /* expected */ }
        other => panic!("expected Ok for destroy, got: {other:?}"),
    }

    let result = engine.execute(CoreOperation::CreateRealm {
        name: "temp-realm".to_string(),
    });
    match result {
        CoreOperationResult::RealmInfo { name, .. } => {
            assert_eq!(name, "temp-realm");
        }
        other => panic!("expected RealmInfo after re-create, got: {other:?}"),
    }
}

#[test]
fn destroy_realm_rejects_unknown_name() {
    let ws = TempWorkspace::new("destroy-unknown-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::DestroyRealm {
        name: "nonexistent".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[test]
fn destroy_realm_rejects_default_realm() {
    let ws = TempWorkspace::new("destroy-default-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::DestroyRealm {
        name: "default".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected: cannot destroy default realm */ }
        other => panic!("expected error for destroying default realm, got: {other:?}"),
    }
}

#[test]
fn add_root_indexes_markdown_files_in_realm() {
    let ws = TempWorkspace::new("add-root");
    let sub = ws.root().join("docs");
    fs::create_dir_all(&sub).expect("subdirectory should be created");
    fs::write(sub.join("a.md"), "# Alpha\n").expect("a.md should be created");
    fs::write(sub.join("b.md"), "# Beta\n").expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "docs-realm".to_string(),
    });

    let result = engine.execute(CoreOperation::AddRoot {
        realm: "docs-realm".to_string(),
        root: sub,
    });

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "docs-realm");
            assert_eq!(root_count, 1);
            assert_eq!(document_count, 2);
        }
        other => panic!("expected RealmInfo, got: {other:?}"),
    }
}

#[test]
fn add_root_rejects_unknown_realm() {
    let ws = TempWorkspace::new("add-root-unknown");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::AddRoot {
        realm: "nonexistent".to_string(),
        root: ws.root(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[test]
fn add_root_rejects_invalid_path() {
    let ws = TempWorkspace::new("add-root-invalid");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "r".to_string(),
    });

    let result = engine.execute(CoreOperation::AddRoot {
        realm: "r".to_string(),
        root: PathBuf::from("/nonexistent/path/to/nowhere"),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for invalid path, got: {other:?}"),
    }
}

#[test]
fn add_root_rejects_duplicate_root() {
    let ws = TempWorkspace::new("add-root-dup");
    fs::write(ws.root().join("a.md"), "# A\n").expect("a.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "r".to_string(),
    });
    let _ = engine.execute(CoreOperation::AddRoot {
        realm: "r".to_string(),
        root: ws.root(),
    });

    let result = engine.execute(CoreOperation::AddRoot {
        realm: "r".to_string(),
        root: ws.root(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for duplicate root, got: {other:?}"),
    }
}

#[test]
fn remove_root_unindexes_documents() {
    let ws = TempWorkspace::new("remove-root");
    let docs = ws.root().join("docs");
    fs::create_dir_all(&docs).expect("docs dir should be created");
    fs::write(docs.join("x.md"), "# X\n").expect("x.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "r".to_string(),
    });
    let _ = engine.execute(CoreOperation::AddRoot {
        realm: "r".to_string(),
        root: docs.clone(),
    });

    let result = engine.execute(CoreOperation::RemoveRoot {
        realm: "r".to_string(),
        root: docs,
    });

    match result {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => {
            assert_eq!(name, "r");
            assert_eq!(root_count, 0);
            assert_eq!(document_count, 0);
        }
        other => panic!("expected RealmInfo after remove, got: {other:?}"),
    }
}

#[test]
fn remove_root_rejects_unknown_realm() {
    let ws = TempWorkspace::new("remove-root-unknown-realm");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::RemoveRoot {
        realm: "nonexistent".to_string(),
        root: ws.root(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for unknown realm, got: {other:?}"),
    }
}

#[test]
fn remove_root_rejects_untracked_root() {
    let ws = TempWorkspace::new("remove-root-untracked");
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let _ = engine.execute(CoreOperation::CreateRealm {
        name: "r".to_string(),
    });

    let result = engine.execute(CoreOperation::RemoveRoot {
        realm: "r".to_string(),
        root: ws.root(),
    });

    match result {
        CoreOperationResult::Error(_) => { /* expected */ }
        other => panic!("expected error for untracked root, got: {other:?}"),
    }
}

#[test]
fn skips_non_utf8_documents_without_failing_startup() {
    let ws = TempWorkspace::new("invalid-utf8");
    let good = ws.root().join("good.md");
    let bad = ws.root().join("bad.md");
    fs::write(&good, "# Intro\n").expect("valid markdown should be created");
    fs::write(&bad, [0xFFu8, 0xFEu8, 0xFDu8]).expect("invalid utf8 markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let outline = engine.execute(CoreOperation::GetOutline {
        uri: DocumentUri::from_file_path(&good),
    });
    match outline {
        CoreOperationResult::Outline(headings) => assert_eq!(headings, vec!["Intro"]),
        other => panic!("expected outline result, got: {other:?}"),
    }
}

// --- realm-stats integration tests ---

#[test]
fn realm_stats_returns_aggregate_counts_for_default_realm() {
    let ws = TempWorkspace::new("realm-stats");
    let doc1 = ws.root().join("notes.md");
    let doc2 = ws.root().join("links.md");
    fs::write(
        &doc1,
        "# Heading A\n\n## Heading B\n\n<agent>content</agent>\n",
    )
    .expect("doc1 should be created");
    fs::write(
        &doc2,
        "# Another\n\n[[notes]]\n\n[Click](https://example.com)\n",
    )
    .expect("doc2 should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "default".to_string(),
    });

    match result {
        CoreOperationResult::RealmStats {
            name,
            root_count,
            document_count,
            heading_count,
            xml_tag_count,
            wiki_link_count,
            markdown_link_count,
        } => {
            assert_eq!(name, "default");
            assert_eq!(root_count, 1);
            assert_eq!(document_count, 2);
            assert_eq!(heading_count, 3);
            assert!(xml_tag_count >= 1, "expected at least 1 XML tag");
            assert_eq!(wiki_link_count, 1);
            assert_eq!(markdown_link_count, 1);
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
    }
}

#[test]
fn realm_stats_errors_for_nonexistent_realm() {
    let ws = TempWorkspace::new("realm-stats-missing");
    fs::write(ws.root().join("a.md"), "# A\n").expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "nonexistent".to_string(),
    });

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error result, got: {other:?}"),
    }
}

#[test]
fn realm_stats_works_for_empty_realm() {
    let engine = RuntimeEngine::default();

    // Create a new empty realm
    engine.execute(CoreOperation::CreateRealm {
        name: "empty-realm".to_string(),
    });

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "empty-realm".to_string(),
    });

    match result {
        CoreOperationResult::RealmStats {
            name,
            root_count,
            document_count,
            heading_count,
            xml_tag_count,
            wiki_link_count,
            markdown_link_count,
        } => {
            assert_eq!(name, "empty-realm");
            assert_eq!(root_count, 0);
            assert_eq!(document_count, 0);
            assert_eq!(heading_count, 0);
            assert_eq!(xml_tag_count, 0);
            assert_eq!(wiki_link_count, 0);
            assert_eq!(markdown_link_count, 0);
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
    }
}

// --- export-index integration tests ---

#[test]
fn export_index_returns_full_document_data() {
    let ws = TempWorkspace::new("export-index");
    let doc = ws.root().join("notes.md");
    fs::write(
        &doc,
        "# Introduction\n\n## Details\n\n<agent>stuff</agent>\n\n[[other-page#section]]\n\n[Click](https://example.com)\n",
    )
    .expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine.execute(CoreOperation::ExportIndex { uri: uri.clone() });

    match result {
        CoreOperationResult::DocumentExport {
            uri: result_uri,
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
        } => {
            assert_eq!(result_uri.as_str(), uri.as_str());
            assert_eq!(headings.len(), 2);
            assert_eq!(headings[0].0, "Introduction");
            assert_eq!(headings[0].1, 1); // level
            assert_eq!(headings[1].0, "Details");
            assert_eq!(headings[1].1, 2); // level
            assert!(!xml_tags.is_empty(), "expected at least 1 XML tag");
            assert_eq!(xml_tags[0].0, "agent");
            assert_eq!(wiki_links.len(), 1);
            assert_eq!(wiki_links[0].0, "other-page");
            assert_eq!(wiki_links[0].1, Some("section".to_string()));
            assert_eq!(markdown_links.len(), 1);
            assert_eq!(markdown_links[0].0, "Click");
            assert_eq!(markdown_links[0].1, "https://example.com");
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

#[test]
fn export_index_errors_for_unindexed_document() {
    let ws = TempWorkspace::new("export-index-missing");
    fs::write(ws.root().join("a.md"), "# A\n").expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::ExportIndex {
        uri: DocumentUri::from_file_path(&ws.root().join("nonexistent.md")),
    });

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error result, got: {other:?}"),
    }
}

#[test]
fn export_index_returns_empty_lists_for_minimal_document() {
    let ws = TempWorkspace::new("export-index-minimal");
    let doc = ws.root().join("empty.md");
    fs::write(&doc, "Just some text with no structure.\n").expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine.execute(CoreOperation::ExportIndex { uri: uri.clone() });

    match result {
        CoreOperationResult::DocumentExport {
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
            ..
        } => {
            assert!(headings.is_empty());
            assert!(xml_tags.is_empty());
            assert!(wiki_links.is_empty());
            assert!(markdown_links.is_empty());
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}
