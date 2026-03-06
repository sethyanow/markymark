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
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "expected Outline from named realm, got {result:?}"
    );
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
