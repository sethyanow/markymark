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
mod engine_indexing;
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
