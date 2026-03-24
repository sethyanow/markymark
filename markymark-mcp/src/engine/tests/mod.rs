use super::*;
use markymark_core::{Position, Range};
use std::fs;

fn make_temp_realm_dir(_suffix: &str) -> tempfile::TempDir {
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
mod recommend;

#[cfg(feature = "semantic-search")]
mod preview_profiling;

#[tokio::test]
async fn get_outline_uses_named_realm() {
    let dir = make_temp_realm_dir("get-outline");
    fs::write(dir.path().join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
    let engine = make_engine_with_custom_realm("my-realm", dir.path()).await;

    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    // Should fail without realm (default realm has no such doc)
    let result = engine
        .execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: None,
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error when querying default realm, got {result:?}"
    );

    // Should succeed with the correct realm
    let result = engine
        .execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: Some("my-realm".to_string()),
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "expected Outline from named realm, got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Outline tree format tests (marky-bgtt)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn outline_flat_format_backward_compat() {
    let dir = make_temp_realm_dir("outline-flat");
    fs::write(dir.path().join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
    let engine = make_engine_with_custom_realm("flat-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("flat-realm".to_string()),
            format: "flat".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::Outline(headings) => {
            assert_eq!(headings, vec!["Hello World", "Section"]);
        }
        other => panic!("expected flat Outline, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_nested_hierarchy() {
    let dir = make_temp_realm_dir("outline-tree");
    fs::write(
        dir.path().join("doc.md"),
        "# Root\n\n## Child\n\n### Grandchild\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("tree-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("tree-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            // Root node (level 0, no heading)
            assert_eq!(tree.title, "");
            assert_eq!(tree.level, 0);
            assert_eq!(tree.children.len(), 1, "root should have 1 h1 child");

            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Root");
            assert_eq!(h1.level, 1);
            assert_eq!(h1.children.len(), 1, "h1 should have 1 h2 child");

            let h2 = &h1.children[0];
            assert_eq!(h2.title, "Child");
            assert_eq!(h2.level, 2);
            assert_eq!(h2.children.len(), 1, "h2 should have 1 h3 child");

            let h3 = &h2.children[0];
            assert_eq!(h3.title, "Grandchild");
            assert_eq!(h3.level, 3);
            assert!(h3.children.is_empty());
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_skipped_levels() {
    // h1 followed by h3 (no h2) should still produce correct hierarchy
    let dir = make_temp_realm_dir("outline-skip");
    fs::write(
        dir.path().join("doc.md"),
        "# Top\n\n### Deep\n\n## Middle\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("skip-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("skip-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.children.len(), 1, "root should have 1 h1 child");
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Top");
            // h3 is child of h1, h2 is sibling of h3 (comes after, higher level)
            assert_eq!(h1.children.len(), 2, "h1 should have h3 and h2 as children");
            assert_eq!(h1.children[0].title, "Deep");
            assert_eq!(h1.children[0].level, 3);
            assert_eq!(h1.children[1].title, "Middle");
            assert_eq!(h1.children[1].level, 2);
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_format_no_headings() {
    let dir = make_temp_realm_dir("outline-empty");
    fs::write(dir.path().join("doc.md"), "Just some text, no headings.\n").unwrap();
    let engine = make_engine_with_custom_realm("empty-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("empty-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.title, "");
            assert_eq!(tree.level, 0);
            assert!(tree.children.is_empty(), "no headings = no children");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_tree_root_node_no_heading() {
    // Root OutlineNode has heading=None; verify it serializes correctly
    let dir = make_temp_realm_dir("outline-root");
    fs::write(dir.path().join("doc.md"), "# Title\n").unwrap();
    let engine = make_engine_with_custom_realm("root-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("root-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.title, "", "root node should have empty title");
            assert_eq!(tree.level, 0, "root node should be level 0");
            assert!(tree.text.is_none(), "no include_text = no text field");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_include_text_false_omits_field() {
    let dir = make_temp_realm_dir("outline-notext");
    fs::write(dir.path().join("doc.md"), "# Title\n\nSome content here.\n").unwrap();
    let engine = make_engine_with_custom_realm("notext-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("notext-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert!(tree.text.is_none(), "root should have no text");
            let h1 = &tree.children[0];
            assert!(
                h1.text.is_none(),
                "h1 should have no text when include_text=false"
            );
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_include_text_true_inlines_content() {
    let dir = make_temp_realm_dir("outline-text");
    fs::write(
        dir.path().join("doc.md"),
        "# Title\n\nParagraph one.\n\n## Section\n\nParagraph two.\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("text-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("text-realm".to_string()),
            format: "tree".to_string(),
            include_text: true,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "Title");
            let text = h1.text.as_ref().expect("h1 should have text");
            assert!(
                text.contains("Paragraph one."),
                "h1 text should contain paragraph one, got: {text}"
            );
            // h1 text should NOT contain h2's content (section boundary)
            assert!(
                !text.contains("Paragraph two."),
                "h1 text should not contain paragraph two"
            );

            let h2 = &h1.children[0];
            assert_eq!(h2.title, "Section");
            let text2 = h2.text.as_ref().expect("h2 should have text");
            assert!(
                text2.contains("Paragraph two."),
                "h2 text should contain paragraph two, got: {text2}"
            );
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn outline_structured_doc_tree_fallback_to_flat() {
    // JSON/YAML documents should return flat format even when tree requested
    let dir = make_temp_realm_dir("outline-structured");
    fs::write(
        dir.path().join("config.json"),
        r#"{"key": "value", "nested": {"inner": 42}}"#,
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("struct-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("config.json"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("struct-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    // Structured docs should fall back to flat format
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "structured doc with format=tree should fall back to flat, got {result:?}"
    );
}

#[tokio::test]
async fn outline_unicode_headings() {
    let dir = make_temp_realm_dir("outline-unicode");
    fs::write(
        dir.path().join("doc.md"),
        "# 日本語タイトル\n\n## Über-docs\n\n### 🎉 Emoji\n",
    )
    .unwrap();
    let engine = make_engine_with_custom_realm("unicode-realm", dir.path()).await;
    let uri = DocumentUri::from_file_path(&dir.path().join("doc.md"));

    let result = engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: Some("unicode-realm".to_string()),
            format: "tree".to_string(),
            include_text: false,
        })
        .await;
    match result {
        CoreOperationResult::OutlineTree(tree) => {
            assert_eq!(tree.children.len(), 1);
            let h1 = &tree.children[0];
            assert_eq!(h1.title, "日本語タイトル");
            assert_eq!(h1.children[0].title, "Über-docs");
            assert_eq!(h1.children[0].children[0].title, "🎉 Emoji");
        }
        other => panic!("expected OutlineTree, got {other:?}"),
    }
}

#[tokio::test]
async fn export_index_uses_named_realm() {
    let dir = make_temp_realm_dir("export-index");
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
    let dir = make_temp_realm_dir("search-symbols");
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
    let dir = make_temp_realm_dir("find-refs");
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
    let dir = make_temp_realm_dir("rename");
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
    let dir = make_temp_realm_dir("find-refs-structured-key");
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
    let dir = make_temp_realm_dir("find-refs-structured-off-key");
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
    let dir = make_temp_realm_dir("rename-structured");
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

#[tokio::test]
async fn collect_documents_includes_json_alongside_markdown() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("notes.md"), "# Hello\n").unwrap();
    fs::write(dir.path().join("config.json"), "{}").unwrap();
    fs::write(dir.path().join("settings.yaml"), "key: val\n").unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let kinds: Vec<_> = docs.iter().map(|(_, k)| *k).collect();

    assert!(kinds.contains(&DocumentKind::Markdown));
    assert!(kinds.contains(&DocumentKind::Json));
    assert!(kinds.contains(&DocumentKind::Yaml));
    // main.rs should NOT be collected
    assert_eq!(docs.len(), 3);
}

// ---------------------------------------------------------------------------
// HashEmbeddingProvider tests (semantic-search feature required)
// ---------------------------------------------------------------------------

/// fnv1a32 must produce the same u32 for the same bytes every time.
///
/// This pins the hash algorithm choice: `DefaultHasher` (SipHash 1-3) is
/// explicitly not stable across Rust versions per std docs.  FNV-1a 32-bit
/// is a fixed, well-specified algorithm that produces identical output
/// forever for the same input.
///
/// The constant 0x4f9f2cab is the standard FNV-1a 32-bit hash of "hello"
/// (verified against the reference implementation and online calculators).
#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn fnv1a32_is_stable_and_deterministic() {
    let h1 = fnv1a32(b"hello");
    let h2 = fnv1a32(b"hello");
    assert_eq!(h1, h2, "same input must produce same hash");
    assert_ne!(
        fnv1a32(b"hello"),
        fnv1a32(b"world"),
        "distinct tokens must hash differently"
    );
    // Pin the exact value (verified against FNV-1a 32-bit reference implementation).
    assert_eq!(
        fnv1a32(b"hello"),
        0x4f9f2cab,
        "FNV-1a 32-bit hash of 'hello' must be 0x4f9f2cab"
    );
    // Empty string -> offset basis unchanged
    assert_eq!(
        fnv1a32(b""),
        0x811c9dc5,
        "empty bytes must return FNV offset basis"
    );
}

/// HashEmbeddingProvider must produce a normalized output vector of the
/// expected dimensionality.
#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn hash_embedding_output_is_normalized_and_correct_dims() {
    let provider = HashEmbeddingProvider::new(128);
    let emb = provider.embed("hello world").await.unwrap();
    assert_eq!(emb.len(), 128, "embedding length must match dims");
    let norm_sq: f32 = emb.iter().map(|v| v * v).sum();
    assert!(
        (norm_sq - 1.0).abs() < 1e-5,
        "embedding must be L2-normalised, got norm^2={norm_sq}"
    );
}

/// HashEmbeddingProvider must produce identical vectors for identical input.
/// This test detects accidental use of randomised hashing (e.g. RandomState).
#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn hash_embedding_is_deterministic() {
    let provider = HashEmbeddingProvider::new(64);
    let a = provider.embed("markymark semantic search").await.unwrap();
    let b = provider.embed("markymark semantic search").await.unwrap();
    assert_eq!(a, b, "identical input must produce identical embedding");
}

/// Empty text must fail with InvalidInput (not panic).
#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn hash_embedding_rejects_empty_text() {
    let provider = HashEmbeddingProvider::new(32);
    let err = provider.embed("   ").await.unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "whitespace-only input must return InvalidInput, got {err:?}"
    );
}

/// Zero dims must fail with InvalidInput (not divide-by-zero).
#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn hash_embedding_rejects_zero_dims() {
    let provider = HashEmbeddingProvider::new(0);
    let err = provider.embed("hello").await.unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "zero dims must return InvalidInput, got {err:?}"
    );
}

/// MCP batch-indexed markdown documents must have code spans extracted.
///
/// This tests the B-8 migration: from_ast → from_scan for MCP batch indexing.
/// The `from_scan` path (Zig extraction) extracts inline code spans, while
/// `from_ast` does not. After migration, searching for code span text should
/// return results.
#[tokio::test]
async fn batch_indexed_docs_have_code_spans() {
    let dir = make_temp_realm_dir("code-spans");
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
    let dir = make_temp_realm_dir("frontmatter-preservation");
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

#[tokio::test]
async fn collect_documents_markdown_unchanged() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("readme.md"), "# R\n").unwrap();
    fs::write(dir.path().join("guide.markdown"), "# G\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    assert_eq!(docs.len(), 2);
    assert!(docs.iter().all(|(_, k)| *k == DocumentKind::Markdown));
}

// -- Engine-based indexing tests (Phase 3: marky-xfgb) --

#[tokio::test]
async fn engine_index_creates_persistent_engines() {
    let dir = make_temp_realm_dir("engine-creates");
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
    let dir = make_temp_realm_dir("update-fail");
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

#[tokio::test]
async fn engine_fallback_scan_when_no_stale_state() {
    let dir = make_temp_realm_dir("create-fail");
    // Magic filename triggers forced create failure — no engine created.
    let path = dir.path().join("__marky_test_force_create_fail__.md");
    fs::write(&path, "# Scan Fallback\n\nShould use scan path.\n").unwrap();

    let mut realm = RealmData::new();

    // First index: engine create forced to fail, no stale state exists.
    // Should fall back to scan path and still produce a valid index.
    index_root_into_realm(dir.path(), &mut realm).await;

    // No engine should be created (create was forced to fail).
    assert_eq!(
        realm.engines.len(),
        0,
        "no engine should be created when create is forced to fail"
    );

    // But the document should still be indexed via scan fallback.
    assert_eq!(
        realm.index.document_count(),
        1,
        "scan fallback should produce a document index"
    );

    // Verify content — scan path should extract headings.
    let uri = DocumentUri::from_file_path(&path);
    let doc = realm.index.get_document(&uri);
    assert!(
        doc.is_some(),
        "document should be retrievable via scan fallback"
    );
    assert!(
        !doc.unwrap().headings().is_empty(),
        "scan fallback document should have headings"
    );
}

#[tokio::test]
async fn engine_cleanup_on_root_removal() {
    let dir = make_temp_realm_dir("cleanup");
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
    let dir = make_temp_realm_dir("frontmatter-engine");
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
