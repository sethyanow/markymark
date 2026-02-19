//! Integration tests for `RuntimeEngine` (the MCP core engine implementation).
//!
//! Extracted from `src/runtime_engine.rs` during the z6r refactor.

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use markymark_mcp::RuntimeEngine;

#[path = "runtime_engine_tests/startup.rs"]
mod startup;

/// Compare two ranges for deterministic sorting (test-local copy).
pub(crate) fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
}

pub(crate) struct TempWorkspace {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl TempWorkspace {
    pub(crate) fn new(name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("markymark-mcp-runtime-{name}-"))
            .tempdir()
            .expect("secure temporary workspace directory should be created");
        let root = dir.path().to_path_buf();
        Self { _dir: dir, root }
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.root.clone()
    }
}

#[test]
fn search_symbols_prefers_prefix_over_plain_substring() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-prefix");
    let first = ws.root().join("a.md");
    let second = ws.root().join("b.md");

    fs::write(&first, "# setup\n# stage\n").expect("first markdown should be created");
    fs::write(&second, "# close\n").expect("second markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "st".to_string(),
        realm: None,
    });

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stage".to_string(), "setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[test]
fn search_symbols_matches_case_insensitively() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-case");
    let file = ws.root().join("case.md");
    fs::write(&file, "# Setup\n# stage\n").expect("markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "ST".to_string(),
        realm: None,
    });

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stage".to_string(), "Setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[test]
fn search_symbols_supports_subsequence_matching() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-subseq");
    let file = ws.root().join("subseq.md");
    fs::write(&file, "# setup\n# stop\n").expect("markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "stp".to_string(),
        realm: None,
    });

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stop".to_string(), "setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[test]
fn search_symbols_returns_no_results_when_query_cannot_be_matched() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-none");
    let file = ws.root().join("none.md");
    fs::write(&file, "# setup\n# stage\n").expect("markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "zzz".to_string(),
        realm: None,
    });

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            assert!(matches.is_empty(), "expected no fuzzy matches");
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[test]
fn search_symbols_uses_batch_ranked_results_ordering() {
    let ws = TempWorkspace::new("search-symbols-batch-ranked-order");
    let file = ws.root().join("ranked.md");
    fs::write(&file, "# acb\n# adb\n# aeb\n").expect("markdown should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let symbols = engine.execute(CoreOperation::SearchSymbols {
        query: "ab".to_string(),
        realm: None,
    });

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(
                names,
                vec!["acb".to_string(), "adb".to_string(), "aeb".to_string()]
            );
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

// Regression test for: fuzzy_match_batch path lacked alphabetical tie-breaking.
// Candidates from multiple files arrive in HashMap iteration order (non-deterministic).
// When all scores are equal, the sort must fall back to name ASC so the result is
// stable regardless of which document the realm iterates first.
// Bug introduced in feat(marky-8xt), fixed by adding the same comparator used in
// the single-candidate fallback path.
#[test]
fn search_symbols_batch_path_uses_alphabetical_tiebreak_across_files() {
    let ws = TempWorkspace::new("batch-tiebreak-cross-file");
    // Three files, one heading each. All headings contain 'a', so all score equally
    // for query "a". The correct result is alphabetical regardless of which file the
    // realm's HashMap happens to iterate first.
    fs::write(ws.root().join("c.md"), "# Zebra\n").expect("c.md");
    fs::write(ws.root().join("a.md"), "# Alpha\n").expect("a.md");
    fs::write(ws.root().join("b.md"), "# Beta\n").expect("b.md");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "a".to_string(),
        realm: None,
    });

    match result {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            // Must be alphabetical: Alpha < Beta < Zebra.
            // Before the fix this returned in HashMap iteration order, e.g. ["Zebra", "Alpha", "Beta"].
            assert_eq!(
                names,
                vec!["Alpha".to_string(), "Beta".to_string(), "Zebra".to_string()],
                "batch path must use alphabetical tiebreak; got {names:?}"
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        realm: None,
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
        check_duplicates: false,
        include_token_counts: false,
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
            ..
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
        check_duplicates: false,
        include_token_counts: false,
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
        check_duplicates: false,
        include_token_counts: false,
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
            ..
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

#[test]
fn realm_stats_can_include_token_estimate() {
    let ws = TempWorkspace::new("realm-stats-token-estimate");
    fs::write(ws.root().join("notes.md"), "# Intro\nsome words here\n")
        .expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "default".to_string(),
        check_duplicates: false,
        include_token_counts: true,
    });

    match result {
        CoreOperationResult::RealmStats { total_tokens, .. } => {
            assert!(
                total_tokens.unwrap_or(0) > 0,
                "expected token estimate to be present"
            );
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
    }
}

#[cfg(feature = "semantic-search")]
#[test]
fn semantic_search_returns_ranked_matches() {
    let ws = TempWorkspace::new("semantic-search-default-realm");
    let intro = ws.root().join("intro.md");
    let setup = ws.root().join("setup.md");
    fs::write(&intro, "# Introduction\n\nA short overview.\n").expect("intro doc should exist");
    fs::write(&setup, "# Installation\n\nSetup steps.\n").expect("setup doc should exist");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::SemanticSearch {
        query: "introduction overview".to_string(),
        realm: None,
        top_k: 3,
        min_score: 0.0,
    });

    match result {
        CoreOperationResult::SemanticMatches(matches) => {
            assert!(!matches.is_empty(), "expected at least one semantic match");
            assert_eq!(matches[0].heading, "Introduction");
            assert!(matches[0].score > 0.0);
            assert!(!matches[0].section_preview.is_empty());
            assert!(
                matches[0].section_preview.len() <= 200,
                "preview should be truncated to 200 chars"
            );
        }
        other => panic!("expected SemanticMatches result, got: {other:?}"),
    }
}

#[cfg(feature = "semantic-search")]
#[test]
fn semantic_search_preview_stays_within_200_bytes_for_unicode() {
    let ws = TempWorkspace::new("semantic-search-unicode-preview");
    let unicode_doc = ws.root().join("unicode.md");
    let long_emoji = "😀".repeat(260);
    fs::write(&unicode_doc, format!("# Unicode\n\n{}\n", long_emoji))
        .expect("unicode markdown should exist");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::SemanticSearch {
        query: "unicode".to_string(),
        realm: None,
        top_k: 1,
        min_score: 0.0,
    });

    match result {
        CoreOperationResult::SemanticMatches(matches) => {
            assert!(!matches.is_empty(), "expected at least one semantic match");
            assert!(
                matches[0].section_preview.len() <= 200,
                "preview should be truncated to <= 200 bytes"
            );
        }
        other => panic!("expected SemanticMatches result, got: {other:?}"),
    }
}

#[test]
fn realm_stats_token_count_is_none_when_source_files_are_missing() {
    let ws = TempWorkspace::new("realm-stats-missing-source");
    let doc = ws.root().join("missing-after-index.md");
    fs::write(&doc, "# Title\n\nsome content\n").expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");
    fs::remove_file(&doc).expect("doc should be removable after indexing");

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "default".to_string(),
        check_duplicates: false,
        include_token_counts: true,
    });

    match result {
        CoreOperationResult::RealmStats { total_tokens, .. } => {
            assert!(
                total_tokens.is_none(),
                "token count should be omitted when indexed files are unreadable"
            );
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
    }
}

// --- realm isolation tests ---

/// Regression test for marky-uux: search-symbols must not return results from a
/// different realm. Both realms exist simultaneously; querying realm_a must not
/// surface symbols that only exist in realm_b.
#[test]
fn search_symbols_scopes_results_to_realm() {
    let ws_a = TempWorkspace::new("realm-isolate-a");
    let ws_b = TempWorkspace::new("realm-isolate-b");

    // Each realm has a document with a heading that is unique to that realm.
    fs::write(ws_a.root().join("a.md"), "# HeadingOnlyInRealmAlpha\n")
        .expect("a.md should be created");
    fs::write(ws_b.root().join("b.md"), "# HeadingOnlyInRealmBeta\n")
        .expect("b.md should be created");

    let engine = RuntimeEngine::default();

    engine.execute(CoreOperation::CreateRealm {
        name: "realm-alpha".to_string(),
    });
    engine.execute(CoreOperation::AddRoot {
        realm: "realm-alpha".to_string(),
        root: ws_a.root(),
    });

    engine.execute(CoreOperation::CreateRealm {
        name: "realm-beta".to_string(),
    });
    engine.execute(CoreOperation::AddRoot {
        realm: "realm-beta".to_string(),
        root: ws_b.root(),
    });

    // Searching realm-alpha for the beta-unique heading must return empty.
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "HeadingOnlyInRealmBeta".to_string(),
        realm: Some("realm-alpha".to_string()),
    });
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                matches.is_empty(),
                "realm-alpha search must not return realm-beta symbol; got: {matches:?}"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }

    // Searching realm-beta for the alpha-unique heading must also return empty.
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "HeadingOnlyInRealmAlpha".to_string(),
        realm: Some("realm-beta".to_string()),
    });
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                matches.is_empty(),
                "realm-beta search must not return realm-alpha symbol; got: {matches:?}"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }

    // Each realm correctly finds its own symbol.
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "HeadingOnlyInRealmAlpha".to_string(),
        realm: Some("realm-alpha".to_string()),
    });
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                !matches.is_empty(),
                "realm-alpha search must find its own symbol"
            );
        }
        other => panic!("expected Symbols result, got: {other:?}"),
    }
}

/// Regression test for marky-uux: after destroy-realm, search-symbols for that
/// realm must return an error (realm no longer exists), not stale results.
#[test]
fn search_symbols_returns_empty_after_destroy_realm() {
    let ws = TempWorkspace::new("realm-destroy-symbols");
    fs::write(ws.root().join("doc.md"), "# UniqueHeadingForDestroyTest\n")
        .expect("doc.md should be created");

    let engine = RuntimeEngine::default();

    engine.execute(CoreOperation::CreateRealm {
        name: "transient-realm".to_string(),
    });
    engine.execute(CoreOperation::AddRoot {
        realm: "transient-realm".to_string(),
        root: ws.root(),
    });

    // Verify symbol is found before destroy.
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "UniqueHeadingForDestroyTest".to_string(),
        realm: Some("transient-realm".to_string()),
    });
    match result {
        CoreOperationResult::Symbols(matches) => {
            assert!(
                !matches.is_empty(),
                "symbol should be found before realm is destroyed"
            );
        }
        other => panic!("expected Symbols result before destroy, got: {other:?}"),
    }

    // Destroy the realm.
    let destroy = engine.execute(CoreOperation::DestroyRealm {
        name: "transient-realm".to_string(),
    });
    assert!(
        matches!(destroy, CoreOperationResult::Ok),
        "destroy-realm should succeed; got: {destroy:?}"
    );

    // After destroy, searching for the symbol in that realm must return an error
    // (realm does not exist), not a stale Symbols result.
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "UniqueHeadingForDestroyTest".to_string(),
        realm: Some("transient-realm".to_string()),
    });
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "search-symbols must return error for destroyed realm, not stale results; got: {result:?}"
    );
}

/// Querying a realm that was **never created** must return a structured error
/// (`CoreOperationResult::Error`), not panic or return empty data. This covers
/// every query operation that accepts a realm parameter.
///
/// Regression coverage for marky-w85.
#[test]
fn query_operations_error_on_never_created_realm() {
    let engine = RuntimeEngine::default();
    let bogus = "never-created-realm";
    let dummy_uri = DocumentUri::from_file_path(std::path::Path::new("/tmp/dummy.md"));
    let dummy_pos = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 5,
        },
    };

    let operations: Vec<(&str, CoreOperation)> = vec![
        (
            "SearchSymbols",
            CoreOperation::SearchSymbols {
                query: "anything".to_string(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "GetOutline",
            CoreOperation::GetOutline {
                uri: dummy_uri.clone(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "FindReferences",
            CoreOperation::FindReferences {
                uri: dummy_uri.clone(),
                position: dummy_pos,
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "Rename",
            CoreOperation::Rename {
                uri: dummy_uri.clone(),
                position: dummy_pos,
                new_name: "new".to_string(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "ExportIndex",
            CoreOperation::ExportIndex {
                uri: dummy_uri.clone(),
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "SearchWorkspace",
            CoreOperation::SearchWorkspace {
                query: Some("anything".to_string()),
                frontmatter_filter: None,
                property_filter: None,
                tag_filter: None,
                realm: Some(bogus.to_string()),
                limit: 20,
            },
        ),
        (
            "SearchForPattern",
            CoreOperation::SearchForPattern {
                pattern: "test".to_string(),
                include_glob: None,
                context_lines: 0,
                limit: 10,
                case_insensitive: false,
                realm: Some(bogus.to_string()),
            },
        ),
        (
            "GraphAnalysis",
            CoreOperation::GraphAnalysis {
                realm: Some(bogus.to_string()),
                top_n_hubs: 10,
                include_clusters: false,
            },
        ),
        (
            "SemanticSearch",
            CoreOperation::SemanticSearch {
                query: "anything".to_string(),
                realm: Some(bogus.to_string()),
                top_k: 5,
                min_score: 0.0,
            },
        ),
        (
            "DependencyGraph",
            CoreOperation::DependencyGraph {
                realm: bogus.to_string(),
                format: "json".to_string(),
            },
        ),
    ];

    for (label, op) in operations {
        let result = engine.execute(op);
        match &result {
            CoreOperationResult::Error(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("realm does not exist"),
                    "{label}: error message should mention 'realm does not exist', got: {msg}"
                );
            }
            other => panic!("{label}: expected Error for non-existent realm, got: {other:?}"),
        }
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
    let result = engine.execute(CoreOperation::ExportIndex {
        uri: uri.clone(),
        realm: None,
    });

    match result {
        CoreOperationResult::DocumentExport {
            uri: result_uri,
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
            ..
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
        realm: None,
    });

    match result {
        CoreOperationResult::Error(_) => {} // expected
        other => panic!("expected Error result, got: {other:?}"),
    }
}

#[test]
fn workspace_with_mixed_formats_indexes_all_supported_types() {
    let ws = TempWorkspace::new("mixed-formats");
    fs::write(ws.root().join("notes.md"), "# Notes\n").expect("md should be created");
    fs::write(ws.root().join("config.json"), r#"{"key": "val"}"#).expect("json should be created");
    fs::write(ws.root().join("settings.yaml"), "key: val\n").expect("yaml should be created");
    fs::write(ws.root().join(".env"), "DB_HOST=localhost\n").expect("env should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::RealmStats {
        realm: "default".to_string(),
        check_duplicates: false,
        include_token_counts: false,
    });

    match result {
        CoreOperationResult::RealmStats {
            document_count,
            heading_count,
            structured_doc_count,
            key_path_count,
            ..
        } => {
            assert_eq!(document_count, 4, "1 markdown + 3 structured docs");
            assert_eq!(heading_count, 1, "one heading from notes.md");
            assert_eq!(structured_doc_count, 3, "json + yaml + .env");
            assert!(key_path_count >= 3, "at least one key per structured doc");
        }
        other => panic!("expected RealmStats result, got: {other:?}"),
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
    let result = engine.execute(CoreOperation::ExportIndex {
        uri: uri.clone(),
        realm: None,
    });

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

#[test]
fn export_index_includes_frontmatter() {
    let ws = TempWorkspace::new("export-index-frontmatter");
    let doc = ws.root().join("with-frontmatter.md");
    fs::write(
        &doc,
        "---\nstatus: active\ntags: [rust, mcp]\n---\n# My Doc\n",
    )
    .expect("doc should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let uri = DocumentUri::from_file_path(&doc);
    let result = engine.execute(CoreOperation::ExportIndex {
        uri: uri.clone(),
        realm: None,
    });

    match result {
        CoreOperationResult::DocumentExport {
            frontmatter,
            headings,
            ..
        } => {
            // Verify the heading is present (sanity check)
            assert_eq!(headings.len(), 1);
            assert_eq!(headings[0].0, "My Doc");

            // Verify frontmatter is populated
            assert!(
                !frontmatter.is_empty(),
                "frontmatter should not be empty for a document with YAML frontmatter"
            );

            // Find the 'status' key — scalar value wrapped as single-element vec
            let status_entry = frontmatter
                .iter()
                .find(|(k, _)| k == "status")
                .expect("frontmatter should contain 'status' key");
            assert_eq!(status_entry.1, vec!["active"]);

            // Find the 'tags' key — list value preserved as multi-element vec
            let tags_entry = frontmatter
                .iter()
                .find(|(k, _)| k == "tags")
                .expect("frontmatter should contain 'tags' key");
            assert_eq!(tags_entry.1, vec!["rust", "mcp"]);
        }
        other => panic!("expected DocumentExport result, got: {other:?}"),
    }
}

// --- search-workspace integration tests ---

fn engine_with_workspace_files(
    name: &str,
    files: &[(&str, &str)],
) -> (TempWorkspace, RuntimeEngine) {
    let ws = TempWorkspace::new(name);
    for (filename, content) in files {
        fs::write(ws.root().join(filename), content).expect("test file should be created");
    }
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");
    (ws, engine)
}

fn search_workspace(
    engine: &RuntimeEngine,
    query: Option<&str>,
    fm_filter: Option<(&str, &str)>,
    prop_filter: Option<(&str, &str)>,
    tag_filter: Option<&str>,
    limit: u32,
) -> Vec<markymark_core::engine::WorkspaceSearchResult> {
    let result = engine.execute(markymark_core::engine::CoreOperation::SearchWorkspace {
        query: query.map(str::to_string),
        frontmatter_filter: fm_filter.map(|(k, v)| (k.to_string(), v.to_string())),
        property_filter: prop_filter.map(|(k, v)| (k.to_string(), v.to_string())),
        tag_filter: tag_filter.map(str::to_string),
        realm: None,
        limit,
    });
    match result {
        CoreOperationResult::WorkspaceSearchResults { results, .. } => results,
        other => panic!("expected WorkspaceSearchResults, got: {other:?}"),
    }
}

#[test]
fn search_workspace_returns_empty_for_no_matches() {
    let (_ws, engine) = engine_with_workspace_files(
        "sw-no-match",
        &[
            ("alpha.md", "# Alpha Document\n\nSome content.\n"),
            ("beta.md", "# Beta Document\n\nOther content.\n"),
        ],
    );
    let results = search_workspace(&engine, Some("nonexistent_xyz_abc"), None, None, None, 20);
    assert!(
        results.is_empty(),
        "expected no results for unmatched query"
    );
}

#[test]
fn search_workspace_case_insensitive_query() {
    // Bug caught: case-sensitive match silently drops results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-case",
        &[("notes.md", "# Project Alpha\n\nSome content.\n")],
    );
    let results = search_workspace(&engine, Some("project alpha"), None, None, None, 20);
    assert_eq!(
        results.len(),
        1,
        "lowercase query should match title with mixed case"
    );
    assert!(
        (results[0].score - 1.0).abs() < f32::EPSILON,
        "title match should score 1.0"
    );
    assert!(results[0].matched_fields.contains(&"title".to_string()));
}

#[test]
fn search_workspace_title_match_scores_higher_than_heading_match() {
    // Bug caught: title and heading scoring swapped.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-title-score",
        &[
            ("title-doc.md", "# Query Term\n\nContent.\n"),
            (
                "heading-doc.md",
                "# Other Doc\n\n## Query Term\n\nContent.\n",
            ),
        ],
    );
    let results = search_workspace(&engine, Some("query term"), None, None, None, 20);
    assert_eq!(results.len(), 2, "both docs should match");
    // Title match must rank first (score 1.0 > 0.8).
    assert_eq!(results[0].score, 1.0, "title match should score 1.0");
    assert!(results[0].matched_fields.contains(&"title".to_string()));
    assert!(
        results[1].score <= 0.8 + f32::EPSILON,
        "heading match should score at most 0.8"
    );
    assert!(results[1].matched_fields.contains(&"heading".to_string()));
}

#[test]
fn search_workspace_frontmatter_filter_exact_key_match() {
    // Bug caught: partial key match returning wrong docs ("statue" matching "status" filter).
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-key",
        &[
            ("active.md", "---\nstatus: active\n---\n# Active Doc\n"),
            ("draft.md", "---\nstatus: draft\n---\n# Draft Doc\n"),
        ],
    );
    let results = search_workspace(&engine, None, Some(("status", "active")), None, None, 20);
    assert_eq!(results.len(), 1, "only doc with status=active should match");
    assert!(results[0].title.contains("Active"), "wrong doc returned");
}

#[test]
fn search_workspace_frontmatter_filter_case_insensitive_value() {
    // Bug caught: case-sensitive value comparison drops valid results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-ci",
        &[("doc.md", "---\nstatus: Active\n---\n# Doc\n")],
    );
    let results = search_workspace(&engine, None, Some(("status", "active")), None, None, 20);
    assert_eq!(
        results.len(),
        1,
        "lowercase filter value should match 'Active' frontmatter"
    );
}

#[test]
fn search_workspace_frontmatter_list_value_any_element_matches() {
    // Bug caught: list values collapsed to string fails partial match.
    // Parser handles inline YAML list format: [a, b, c]
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-list",
        &[(
            "doc.md",
            "---\naliases: [Project X, Proj X, PX]\n---\n# Document\n",
        )],
    );
    let results = search_workspace(&engine, None, Some(("aliases", "proj x")), None, None, 20);
    assert_eq!(
        results.len(),
        1,
        "filter should match any element in frontmatter list"
    );
}

#[test]
fn search_workspace_property_filter() {
    // Bug caught: property filter not applied.
    // Logseq properties (key:: value) must appear BEFORE headings in source.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-prop",
        &[
            ("daily.md", "type:: daily\n\n# Daily\n\nSome notes.\n"),
            ("note.md", "type:: note\n\n# Note\n\nSome notes.\n"),
        ],
    );
    let results = search_workspace(&engine, None, None, Some(("type", "daily")), None, 20);
    assert_eq!(results.len(), 1, "only doc with type::daily should match");
    assert!(results[0].title.contains("Daily"), "wrong doc returned");
}

#[test]
fn search_workspace_tag_filter_case_insensitive() {
    // Bug caught: case-sensitive tag matching drops valid results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-tag-ci",
        &[
            ("tagged.md", "# Doc\n\n#Project content here.\n"),
            ("other.md", "# Other\n\n#daily content.\n"),
        ],
    );
    let results = search_workspace(&engine, None, None, None, Some("project"), 20);
    assert_eq!(
        results.len(),
        1,
        "lowercase filter should match #Project tag"
    );
}

#[test]
fn search_workspace_multiple_filters_and_logic() {
    // Bug caught: OR instead of AND logic for multiple filters.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-and-logic",
        &[
            ("a.md", "---\nstatus: active\n---\n# Doc A\n\n#project\n"),
            ("b.md", "---\nstatus: active\n---\n# Doc B\n\n#daily\n"),
        ],
    );
    let results = search_workspace(
        &engine,
        None,
        Some(("status", "active")),
        None,
        Some("project"),
        20,
    );
    assert_eq!(
        results.len(),
        1,
        "only doc matching BOTH status=active AND tag=project should return"
    );
    assert!(results[0].title.contains("Doc A"), "wrong doc returned");
}

#[test]
fn search_workspace_respects_limit() {
    // Bug caught: limit not applied or results sorted wrong direction.
    // Search only covers title and headings, not body prose.
    // Use a heading so all 10 docs match the query.
    let files: Vec<(String, String)> = (0..10)
        .map(|i| {
            (
                format!("doc{i:02}.md"),
                format!("# Document {i}\n\n## Common Query Term\n\nsome content\n"),
            )
        })
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, content)| (name.as_str(), content.as_str()))
        .collect();

    let (_ws, engine) = engine_with_workspace_files("sw-limit", &file_refs);
    let results = search_workspace(&engine, Some("common query term"), None, None, None, 3);
    assert_eq!(results.len(), 3, "limit=3 should return exactly 3 results");
    // Verify descending score order.
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "results should be sorted score DESC"
        );
    }
}

#[test]
fn search_workspace_limit_zero_returns_empty() {
    // Bug caught: limit=0 causes panic or returns all docs.
    let (_ws, engine) =
        engine_with_workspace_files("sw-limit-zero", &[("doc.md", "# Doc\n\nsome content\n")]);
    let results = search_workspace(&engine, None, None, None, None, 0);
    assert!(
        results.is_empty(),
        "limit=0 should return empty results, not error"
    );
}

#[test]
fn search_workspace_empty_realm_returns_empty() {
    // Bug caught: iter_documents on empty realm panics.
    let ws = TempWorkspace::new("sw-empty-realm");
    // No files — empty directory.
    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("empty workspace should index");
    let results = search_workspace(&engine, Some("anything"), None, None, None, 20);
    assert!(
        results.is_empty(),
        "empty realm should return empty results, not error"
    );
}

#[test]
fn search_workspace_no_query_no_filter_returns_all_up_to_limit() {
    // Bug caught: no-filter path broken or no-query path errors.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-no-filter",
        &[
            ("a.md", "# Alpha\n"),
            ("b.md", "# Beta\n"),
            ("c.md", "# Gamma\n"),
        ],
    );
    let results = search_workspace(&engine, None, None, None, None, 10);
    assert_eq!(results.len(), 3, "no filters should return all docs");
    for r in &results {
        assert!(
            (r.score - 1.0).abs() < f32::EPSILON,
            "all docs should score 1.0 with no query"
        );
    }
}

#[test]
fn search_workspace_sort_descending_score_ties_by_uri_ascending() {
    // Bug caught: unstable sort, non-deterministic output across runs.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-sort",
        &[
            // All docs share the same query match (heading), so score=0.8.
            // Tie-break should be URI ascending.
            ("zzz-last.md", "# Other\n\n## Query Term\n"),
            ("aaa-first.md", "# Other\n\n## Query Term\n"),
            ("mmm-mid.md", "# Other\n\n## Query Term\n"),
        ],
    );
    let results = search_workspace(&engine, Some("query term"), None, None, None, 20);
    assert_eq!(results.len(), 3, "all three docs should match");
    // All should have the same score (0.8 for heading match) since no title match.
    for r in &results {
        assert!(
            (r.score - 0.8).abs() < f32::EPSILON,
            "all should score 0.8 for heading match"
        );
    }
    // URIs must be in ascending order (deterministic tie-break).
    let uris: Vec<&str> = results.iter().map(|r| r.uri.as_str()).collect();
    let mut sorted = uris.clone();
    sorted.sort();
    assert_eq!(
        uris, sorted,
        "results with equal score should be sorted by URI ascending"
    );
}

// --- find-references: block_ref support (marky-jrw) ---

const BLOCK_UUID_A: &str = "550e8400-e29b-41d4-a716-446655440000";
const BLOCK_UUID_B: &str = "7f6c1b2a-3d4e-5f60-a7b8-c9d0e1f20304";

#[test]
fn find_references_for_block_ref_returns_all_referencing_docs() {
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

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    // Position cursor inside ((uuid)) in Doc A — line 0, char 3 is inside the UUID text
    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(0, 3), Position::new(0, 3)),
        realm: None,
    });

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

#[test]
fn find_references_block_ref_results_sorted_by_uri_then_range() {
    let ws = TempWorkspace::new("find-refs-blockref-sorted");

    // Create files with names that sort alphabetically: a.md < b.md < c.md
    fs::write(ws.root().join("c.md"), format!("(({BLOCK_UUID_A})) in c\n"))
        .expect("c.md should be created");
    fs::write(ws.root().join("a.md"), format!("(({BLOCK_UUID_A})) in a\n"))
        .expect("a.md should be created");
    fs::write(ws.root().join("b.md"), format!("(({BLOCK_UUID_A})) in b\n"))
        .expect("b.md should be created");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&ws.root().join("a.md")),
        position: Range::new(Position::new(0, 3), Position::new(0, 3)),
        realm: None,
    });

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

#[test]
fn find_references_for_block_id_returns_block_ref_locations() {
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

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

    // Cursor inside ^uuid in Doc A: "some content ^550e8400..."
    // ^  is at position 13, UUID starts at 14; position 16 is inside the UUID
    let result = engine.execute(CoreOperation::FindReferences {
        uri: DocumentUri::from_file_path(&a),
        position: Range::new(Position::new(0, 16), Position::new(0, 16)),
        realm: None,
    });

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
