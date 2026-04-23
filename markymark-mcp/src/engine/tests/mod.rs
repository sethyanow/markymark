use super::*;
use markymark_core::{Position, Range};
use std::fs;

fn make_temp_realm_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

async fn make_engine_with_custom_realm(realm_name: &str, dir: &Path) -> RuntimeEngine {
    let engine = RuntimeEngine::default();
    // create the realm
    engine
        .execute(CoreOperation::CreateRealm {
            name: realm_name.to_string(),
        })
        .await;
    // index the directory into it
    engine
        .execute(CoreOperation::AddRoot {
            realm: realm_name.to_string(),
            root: dir.to_path_buf(),
        })
        .await;
    engine
}

#[cfg(feature = "semantic-search")]
mod concurrency;

mod curation;
mod enrich;
mod export_docs_index;
#[cfg(feature = "semantic-search")]
mod hash_embedding;
mod outline;
mod recommend;
mod workspace_scan;

#[cfg(feature = "semantic-search")]
mod preview_profiling;

#[tokio::test]
async fn export_index_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Title\n").unwrap();
    let engine = make_engine_with_custom_realm("export-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: Some("export-realm".to_string()),
            include_blocks: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::DocumentExport { .. }),
        "expected DocumentExport from named realm, got {result:?}"
    );

    let result_default = engine
        .execute(CoreOperation::ExportIndex {
            uri,
            realm: None,
            include_blocks: false,
        })
        .await;
    assert!(
        matches!(result_default, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result_default:?}"
    );
}

#[tokio::test]
async fn search_symbols_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# UniqueHeadingXYZ\n").unwrap();
    let engine = make_engine_with_custom_realm("search-realm", dir.path()).await;

    // Default realm should return no matches for the unique heading
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: None,
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(
            matches.is_empty(),
            "default realm should not have the heading"
        );
    } else {
        panic!("expected Symbols result");
    }

    // Named realm should find it
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: Some("search-realm".to_string()),
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(!matches.is_empty(), "named realm should have the heading");
    } else {
        panic!("expected Symbols result");
    }
}

#[tokio::test]
async fn find_references_uses_named_realm() {
    let dir = make_temp_realm_dir();
    // A heading with a wiki-link reference in the same file
    fs::write(
        dir.path().join("doc.md"),
        "# My Heading\n\n[[My Heading]]\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let position = markymark_core::Range {
        start: Position {
            line: 0,
            character: 2,
        },
        end: Position {
            line: 0,
            character: 12,
        },
    };

    // Default realm has no such doc
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri: uri.clone(),
            position,
            realm: None,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should find the references
    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            position,
            realm: Some("refs-realm".to_string()),
        })
        .await;
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
}

#[tokio::test]
async fn rename_uses_named_realm() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Old Name\n").unwrap();
    let engine = make_engine_with_custom_realm("rename-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let position = markymark_core::Range {
        start: Position {
            line: 0,
            character: 2,
        },
        end: Position {
            line: 0,
            character: 10,
        },
    };

    // Default realm has no such doc
    let result = engine
        .execute(CoreOperation::Rename {
            uri: uri.clone(),
            position,
            new_name: "New Name".to_string(),
            realm: None,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should work
    let result = engine
        .execute(CoreOperation::Rename {
            uri,
            position,
            new_name: "New Name".to_string(),
            realm: Some("rename-realm".to_string()),
        })
        .await;
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
}

#[tokio::test]
async fn find_references_structured_doc_key_returns_empty_locations() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("config.json"),
        "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-structured", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            position: Range::new(Position::new(2, 5), Position::new(2, 5)),
            realm: Some("refs-structured".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Locations(locations) => {
            assert!(
                locations.is_empty(),
                "structured keys have no cross-doc refs"
            )
        }
        other => panic!("expected empty Locations result, got {other:?}"),
    }
}

#[tokio::test]
async fn find_references_structured_doc_off_key_returns_error() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("config.json"),
        "{\n  \"database\": {\n    \"host\": \"localhost\"\n  }\n}\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("refs-structured-off-key", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::FindReferences {
            uri,
            // Cursor on value text ("localhost"), not on a key.
            position: Range::new(Position::new(2, 15), Position::new(2, 15)),
            realm: Some("refs-structured-off-key".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Error(err) => {
            assert!(
                err.to_string()
                    .contains("no referenceable symbol at position"),
                "expected no-symbol error, got {err:?}"
            );
        }
        other => panic!("expected Error result, got {other:?}"),
    }
}

#[tokio::test]
async fn rename_structured_doc_returns_not_supported_error() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("config.toml"), "host = \"localhost\"\n").unwrap();
    let engine = make_engine_with_custom_realm("rename-structured", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.toml"));

    let result = engine
        .execute(CoreOperation::Rename {
            uri,
            position: Range::new(Position::new(0, 1), Position::new(0, 1)),
            new_name: "server_host".to_string(),
            realm: Some("rename-structured".to_string()),
        })
        .await;

    match result {
        CoreOperationResult::Error(err) => {
            assert!(
                err.to_string()
                    .contains("rename is not supported for structured documents"),
                "expected structured rename unsupported error, got {err:?}"
            );
        }
        other => panic!("expected Error result, got {other:?}"),
    }
}

/// MCP batch-indexed markdown documents must have code spans extracted.
///
/// This tests the B-8 migration: from_ast → from_scan for MCP batch indexing.
/// The `from_scan` path (Zig extraction) extracts inline code spans, while
/// `from_ast` does not. After migration, searching for code span text should
/// return results.
#[tokio::test]
async fn batch_indexed_docs_have_code_spans() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "# Code Spans Test\n\nThe `HashMap` type is a key-value store.\n\nUse `Vec<T>` for lists.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("code-spans-realm", dir.path()).await;

    // Search for code span text — should find matches if code spans are extracted
    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "HashMap".to_string(),
            realm: Some("code-spans-realm".to_string()),
        })
        .await;
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(
            !matches.is_empty(),
            "batch-indexed docs should have code spans: searching for 'HashMap' should find the backtick code span"
        );
    } else {
        panic!("expected Symbols result, got {result:?}");
    }
}

/// MCP batch-indexed markdown documents must preserve frontmatter.
///
/// After B-8 migration to from_scan, frontmatter must still be accessible
/// for search filtering, preview, and export. This tests that the
/// `from_scan_with_frontmatter` constructor correctly preserves frontmatter.
#[tokio::test]
async fn batch_indexed_docs_preserve_frontmatter() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "---\ntitle: Test Document\ntags: [rust, zig]\n---\n\n# Content\n\nSome text here.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("fm-realm", dir.path()).await;

    // Search with frontmatter filter should find the document
    let result = engine
        .execute(CoreOperation::SearchWorkspace {
            query: None,
            realm: Some("fm-realm".to_string()),
            frontmatter_filter: Some(("title".to_string(), "Test Document".to_string())),
            property_filter: None,
            tag_filter: None,
            limit: 10,
        })
        .await;
    if let CoreOperationResult::WorkspaceSearchResults { results, .. } = result {
        assert!(
            !results.is_empty(),
            "frontmatter filtering should find the document after from_scan migration"
        );
    } else {
        panic!("expected WorkspaceSearchResults, got {result:?}");
    }
}

// -- Engine-based indexing tests (Phase 3: marky-xfgb) --

#[tokio::test]
async fn engine_index_creates_persistent_engines() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("one.md"), "# Heading One\n\nSome text.\n").unwrap();
    fs::write(dir.path().join("two.md"), "# Heading Two\n\nMore text.\n").unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Engine path should create a persistent engine per markdown file.
    assert_eq!(
        realm.engines.len(),
        2,
        "expected 2 persistent engines, one per markdown file"
    );

    // Documents should also be indexed (behavioral parity with scan path).
    assert_eq!(realm.index.document_count(), 2);
}

#[tokio::test]
async fn engine_fallback_stale_on_update_failure() {
    let dir = make_temp_realm_dir();
    // Magic filename triggers forced update failure on second index.
    let path = dir.path().join("__marky_test_force_update_fail__.md");
    fs::write(&path, "# Original\n\nFirst version.\n").unwrap();

    let mut realm = RealmData::new();

    // First index: engine created successfully with original content.
    index_root_into_realm(dir.path(), &mut realm).await;
    assert_eq!(realm.engines.len(), 1);
    assert_eq!(realm.index.document_count(), 1);

    // Modify the file — the update will be forced to fail.
    fs::write(&path, "# Changed\n\nSecond version.\n").unwrap();

    // Second index: update fails, should fall back to stale engine snapshot.
    index_root_into_realm(dir.path(), &mut realm).await;

    // Document should still be indexed (stale fallback, not empty).
    assert_eq!(
        realm.index.document_count(),
        1,
        "document should still be indexed via stale engine fallback"
    );

    // Verify content is present — stale snapshot should have the original heading.
    let uri = DocumentUri::from_file_path(&path);
    let doc = realm.index.get_document(&uri);
    assert!(
        doc.is_some(),
        "document should be retrievable after stale fallback"
    );
    assert!(
        !doc.unwrap().headings().is_empty(),
        "stale fallback document should have headings from original parse"
    );
}

/// LTO canary: verifies cross-language ThinLTO eliminates the test-only fault
/// injection in the Zig engine. Under LTO, the magic-filename check is optimized
/// away, so the engine creates successfully. Without LTO, the fault injection
/// fires and this test must be skipped.
///
/// Gated on `MARKYMARK_LTO_ENABLED=1` (set by Bazel LTO configs) rather than
/// `cfg!(debug_assertions)`, which is false in any opt build — not just LTO.
#[tokio::test]
async fn lto_eliminates_fault_injection() {
    // Only run under LTO builds — without LTO the fault injection is live.
    if std::env::var("MARKYMARK_LTO_ENABLED").as_deref() != Ok("1") {
        return;
    }

    let dir = make_temp_realm_dir();
    // Magic filename that triggers forced create failure WITHOUT LTO.
    let path = dir.path().join("__marky_test_force_create_fail__.md");
    fs::write(
        &path,
        "# LTO Canary\n\nEngine should create successfully under LTO.\n",
    )
    .unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Under LTO the fault injection is dead code — engine creates normally.
    assert_eq!(
        realm.engines.len(),
        1,
        "LTO should eliminate the fault injection, allowing engine creation"
    );

    assert_eq!(realm.index.document_count(), 1);

    let uri = DocumentUri::from_file_path(&path);
    let doc = realm.index.get_document(&uri);
    assert!(doc.is_some());
    assert!(!doc.unwrap().headings().is_empty());
}

#[tokio::test]
async fn engine_cleanup_on_root_removal() {
    let dir = make_temp_realm_dir();
    fs::write(dir.path().join("doc.md"), "# Cleanup Test\n\nContent.\n").unwrap();

    let mut realm = RealmData::new();

    // Index the root — engine should be created.
    index_root_into_realm(dir.path(), &mut realm).await;
    assert_eq!(realm.engines.len(), 1, "engine should exist after indexing");
    assert_eq!(realm.index.document_count(), 1);

    // Remove the root — engine should be cleaned up.
    unindex_root_from_realm(dir.path(), &mut realm).await;
    assert_eq!(
        realm.engines.len(),
        0,
        "engine should be removed when root is unindexed"
    );
    assert_eq!(
        realm.index.document_count(),
        0,
        "documents should be removed when root is unindexed"
    );
}

#[tokio::test]
async fn engine_frontmatter_preserved() {
    let dir = make_temp_realm_dir();
    fs::write(
        dir.path().join("doc.md"),
        "---\ntitle: Engine FM Test\ntags: [alpha, beta]\naliases: [efm]\n---\n\n# Content\n\nBody text.\n",
    )
    .unwrap();

    let mut realm = RealmData::new();
    index_root_into_realm(dir.path(), &mut realm).await;

    // Engine should be created (not scan path).
    assert_eq!(realm.engines.len(), 1);

    // Frontmatter should be accessible via search filter.
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));
    let doc = realm
        .index
        .get_document(&uri)
        .expect("document should exist");

    // Verify frontmatter entries are present.
    let fm = doc.frontmatter();
    assert!(
        !fm.is_empty(),
        "frontmatter should be preserved via engine path"
    );
    // Check that the title key is present.
    assert!(
        fm.iter().any(|e| e.key == "title"),
        "frontmatter should contain 'title' key"
    );
}

/// Verify `DocumentIndex::from_text()` produces equivalent output to
/// `fallback_scan_with_frontmatter()` for a mixed markdown document.
///
/// This equivalence test ensures the engine path (from_text) can safely
/// replace the scan path (fallback_scan_with_frontmatter) as the fallback
/// in both MCP and LSP.
#[test]
fn from_text_equivalence_with_fallback_scan_mixed_doc() {
    use markymark_index::DocumentIndex;

    let text = "\
---
title: Equivalence Test
tags: [alpha, beta]
aliases: [eq1, eq2]
---

# First Heading

Some body with a [[wiki link]] and a [markdown link](http://example.com).

## Second Heading {#custom-id}

A paragraph with `inline code` and <custom-tag>content</custom-tag>.

- [ ] Task one
- [x] Task two

> [!note]
> A callout block.

^block-ref-id
";

    let scan_index = fallback_scan_with_frontmatter(text);
    let engine_index = DocumentIndex::from_text(text);

    // Headings: count and text
    let scan_headings: Vec<(&str, u8)> = scan_index
        .headings()
        .iter()
        .map(|h| (h.text, h.level))
        .collect();
    let engine_headings: Vec<(&str, u8)> = engine_index
        .headings()
        .iter()
        .map(|h| (h.text, h.level))
        .collect();
    assert_eq!(
        scan_headings, engine_headings,
        "headings mismatch: scan={scan_headings:?} vs engine={engine_headings:?}"
    );

    // Tags
    let scan_tags: Vec<&str> = scan_index.tags().iter().map(|t| t.name).collect();
    let engine_tags: Vec<&str> = engine_index.tags().iter().map(|t| t.name).collect();
    assert_eq!(
        scan_tags, engine_tags,
        "tags mismatch: scan={scan_tags:?} vs engine={engine_tags:?}"
    );

    // Wiki links
    let scan_wiki: Vec<&str> = scan_index.wiki_links().iter().map(|w| w.target).collect();
    let engine_wiki: Vec<&str> = engine_index.wiki_links().iter().map(|w| w.target).collect();
    assert_eq!(
        scan_wiki, engine_wiki,
        "wiki links mismatch: scan={scan_wiki:?} vs engine={engine_wiki:?}"
    );

    // Markdown links
    let scan_md_links: Vec<(&str, &str)> = scan_index
        .markdown_links()
        .iter()
        .map(|l| (l.text, l.url))
        .collect();
    let engine_md_links: Vec<(&str, &str)> = engine_index
        .markdown_links()
        .iter()
        .map(|l| (l.text, l.url))
        .collect();
    assert_eq!(
        scan_md_links, engine_md_links,
        "markdown links mismatch: scan={scan_md_links:?} vs engine={engine_md_links:?}"
    );

    // Frontmatter keys
    let scan_fm: Vec<&str> = scan_index.frontmatter().iter().map(|f| f.key).collect();
    let engine_fm: Vec<&str> = engine_index.frontmatter().iter().map(|f| f.key).collect();
    assert_eq!(
        scan_fm, engine_fm,
        "frontmatter keys mismatch: scan={scan_fm:?} vs engine={engine_fm:?}"
    );

    // Aliases
    assert_eq!(
        scan_index.aliases(),
        engine_index.aliases(),
        "aliases mismatch"
    );

    // XML tags
    let scan_xml: Vec<&str> = scan_index.xml_tags().iter().map(|x| x.tag_name).collect();
    let engine_xml: Vec<&str> = engine_index.xml_tags().iter().map(|x| x.tag_name).collect();
    assert_eq!(
        scan_xml, engine_xml,
        "xml tags mismatch: scan={scan_xml:?} vs engine={engine_xml:?}"
    );

    // Tasks
    assert_eq!(
        scan_index.tasks().len(),
        engine_index.tasks().len(),
        "task count mismatch"
    );

    // Code spans
    let scan_code: Vec<&str> = scan_index.code_spans().iter().map(|c| c.text).collect();
    let engine_code: Vec<&str> = engine_index.code_spans().iter().map(|c| c.text).collect();
    assert_eq!(
        scan_code, engine_code,
        "code spans mismatch: scan={scan_code:?} vs engine={engine_code:?}"
    );
}

/// Verify equivalence for a frontmatter-only document (no markdown body).
///
/// Adversarial finding: after mask_frontmatter, the entire text is whitespace.
/// Both paths should produce an index with frontmatter but no headings/links.
#[test]
fn from_text_equivalence_frontmatter_only_doc() {
    use markymark_index::DocumentIndex;

    let text = "---\ntitle: Only Frontmatter\ntags: [solo]\n---\n";

    let scan_index = fallback_scan_with_frontmatter(text);
    let engine_index = DocumentIndex::from_text(text);

    // Frontmatter preserved
    let scan_fm: Vec<&str> = scan_index.frontmatter().iter().map(|f| f.key).collect();
    let engine_fm: Vec<&str> = engine_index.frontmatter().iter().map(|f| f.key).collect();
    assert_eq!(
        scan_fm, engine_fm,
        "frontmatter keys mismatch for frontmatter-only doc"
    );

    // No headings, links, etc.
    assert_eq!(scan_index.headings().len(), engine_index.headings().len());
    assert_eq!(
        scan_index.wiki_links().len(),
        engine_index.wiki_links().len()
    );
    assert_eq!(
        scan_index.markdown_links().len(),
        engine_index.markdown_links().len()
    );
}
