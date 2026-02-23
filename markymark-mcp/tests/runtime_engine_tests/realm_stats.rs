use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

#[test]
fn realm_stats_returns_aggregate_counts_for_default_realm() {
    let ws = TempWorkspace::new("realm-stats");
    let doc1 = ws.root().join("notes.md");
    let doc2 = ws.root().join("links.md");
    // XML tags must use block-level HTML (open/close on separate lines) for
    // the Zig md4c extraction path. Inline `<tag>x</tag>` is not extracted.
    fs::write(
        &doc1,
        "# Heading A\n\n## Heading B\n\n<agent>\ncontent\n</agent>\n",
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
                "preview should be truncated to 200 bytes"
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
