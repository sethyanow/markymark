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
        })
        .await;
    assert!(
        matches!(result, CoreOperationResult::DocumentExport { .. }),
        "expected DocumentExport from named realm, got {result:?}"
    );

    let result_default = engine
        .execute(CoreOperation::ExportIndex { uri, realm: None })
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

// -------------------------------------------------------------------------
// preview_for_range I/O profiling
//
// These tests measure the cost of the current full-file-read approach vs a
// streaming BufReader alternative.  They are marked `#[ignore]` so they
// don't run by default.
//
// Run manually:
//   cargo test -p markymark-mcp --features semantic-search \
//       -- preview_io_cost --ignored --nocapture
// -------------------------------------------------------------------------

/// Generate a synthetic markdown corpus of approximately `target_bytes`.
/// Each section is ~130 bytes so 1 MB ~ 7 600 sections ~ 45 600 lines.
#[cfg(feature = "semantic-search")]
fn generate_preview_corpus(target_bytes: usize) -> String {
    let mut doc = String::with_capacity(target_bytes + 512);
    doc.push_str("# Preview I/O Profile Corpus\n\n");
    let mut section = 1usize;
    while doc.len() < target_bytes {
        doc.push_str(&format!("## Section {section}\n\n"));
        doc.push_str("This section exists to test streaming vs full-read preview extraction.\n");
        doc.push_str("Content should be realistic length to exercise I/O paths.\n\n");
        section += 1;
    }
    doc
}

/// Alternative preview extraction using `BufRead::lines()` -- reads only
/// until the target line rather than the whole file.
#[cfg(feature = "semantic-search")]
fn streamed_preview(path: &std::path::Path, target_line: u32, max_bytes: usize) -> String {
    use std::io::BufRead as _;
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let reader = std::io::BufReader::new(file);
    let mut buf = String::with_capacity(max_bytes + 256);
    for (i, line) in reader.lines().enumerate() {
        let Ok(line) = line else { break };
        if i as u32 >= target_line {
            buf.push_str(&line);
            buf.push('\n');
            if buf.len() >= max_bytes {
                break;
            }
        }
    }
    let mut end = buf.len().min(max_bytes);
    while end > 0 && !buf.is_char_boundary(end) {
        end -= 1;
    }
    buf[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Profiles `preview_for_range` (full read) vs `streamed_preview`
/// (BufReader) across file sizes from 10 KB to 5 MB.
///
/// Output columns: file_bytes | target_line | full_read_avg | stream_avg | speedup
///
/// Interpretation: speedup > 1.0 means streaming is faster.  Speedup is
/// meaningful only when files are large enough for I/O to dominate (~>500 KB).
#[cfg(feature = "semantic-search")]
#[tokio::test]
#[ignore = "performance profiling -- run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_large_file --ignored --nocapture"]
async fn preview_io_cost_large_file() {
    use markymark_core::Position;
    use std::time::Instant;

    let dir = make_temp_realm_dir("preview-io-profile");
    const ITERS: u32 = 50;

    eprintln!(
        "\n{:<12} {:<12} {:<16} {:<16} {:<10}",
        "file_bytes", "target_line", "full_read_avg", "stream_avg", "speedup"
    );
    eprintln!("{}", "-".repeat(70));

    for &target_bytes in &[10_000usize, 100_000, 500_000, 1_000_000, 5_000_000] {
        let content = generate_preview_corpus(target_bytes);
        let line_count = content.lines().count() as u32;
        let path = dir.path().join(format!("doc_{target_bytes}.md"));
        fs::write(&path, &content).unwrap();

        let uri = DocumentUri::from_file_path(&path);

        // Target a section 75% into the file (worst-case for streaming too).
        let target_line = line_count * 3 / 4;
        let range = Range {
            start: Position {
                line: target_line,
                character: 0,
            },
            end: Position {
                line: target_line + 6,
                character: 0,
            },
        };

        // Warm up OS page cache for a fair comparison.
        let _ = helpers::preview_for_range(&uri, range, "fallback");
        let _ = streamed_preview(&path, target_line, 200);

        // Measure: current approach (full fs::read_to_string).
        let t0 = Instant::now();
        for _ in 0..ITERS {
            let _ = helpers::preview_for_range(&uri, range, "fallback");
        }
        let full_avg = t0.elapsed() / ITERS;

        // Measure: streaming BufReader approach.
        let t1 = Instant::now();
        for _ in 0..ITERS {
            let _ = streamed_preview(&path, target_line, 200);
        }
        let stream_avg = t1.elapsed() / ITERS;

        let speedup = full_avg.as_nanos() as f64 / stream_avg.as_nanos().max(1) as f64;
        eprintln!(
            "{:<12} {:<12} {:<16?} {:<16?} {:<.2}x",
            target_bytes, target_line, full_avg, stream_avg, speedup
        );
    }
}

// -------------------------------------------------------------------------
// Concurrency: semantic search must NOT block realm write operations
//
// Before this fix (marky-ysv8), the SemanticSearch arm of CoreEngine::execute
// held a tokio::RwLock read guard across the search .await. With a slow
// embedding provider (200ms-2s for a Voyage HTTP round-trip), this blocked
// all realm-level writes (CreateRealm, AddRoot, RemoveRoot, DestroyRealm).
//
// After the fix, the engine clones the Arc<Mutex<SemanticIndex>> and drops
// the outer RwLock before searching, so writes proceed concurrently.
// -------------------------------------------------------------------------

#[cfg(feature = "semantic-search")]
mod concurrency_tests {
    use super::*;
    use async_trait::async_trait;
    use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
    use markymark_core::prelude::{EmbedError, EmbeddingProvider};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Slow embedding provider that simulates a Voyage HTTP round-trip.
    ///
    /// `embed()` signals `embed_started` before sleeping, then delegates to
    /// the inner hash-based provider for deterministic output.
    struct SlowEmbeddingProvider {
        inner: HashEmbeddingProvider,
        delay: Duration,
        embed_started: Arc<tokio::sync::Notify>,
    }

    impl SlowEmbeddingProvider {
        fn new(dims: u32, delay: Duration) -> (Self, Arc<tokio::sync::Notify>) {
            let notify = Arc::new(tokio::sync::Notify::new());
            let provider = Self {
                inner: HashEmbeddingProvider::new(dims),
                delay,
                embed_started: Arc::clone(&notify),
            };
            (provider, notify)
        }
    }

    #[async_trait]
    impl EmbeddingProvider for SlowEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
            self.embed_started.notify_one();
            tokio::time::sleep(self.delay).await;
            self.inner.embed(text).await
        }

        fn dimensions(&self) -> u32 {
            self.inner.dimensions()
        }
    }

    /// Semantic search with a slow provider must not block concurrent write
    /// operations on the realm state.
    ///
    /// This test:
    /// 1. Creates an engine with a slow embedding provider (150ms per embed).
    /// 2. Spawns a SemanticSearch that will hold the inner Mutex for ~150ms.
    /// 3. Concurrently runs a CreateRealm write operation.
    /// 4. Asserts CreateRealm completes quickly (well under the search time).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn semantic_search_does_not_block_realm_writes() {
        let delay = Duration::from_millis(150);
        let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

        let dir = make_temp_realm_dir("concurrency");
        fs::write(dir.path().join("doc.md"), "# Hello World\n\nSome content.\n").unwrap();

        let engine = Arc::new(
            RuntimeEngine::from_workspace_roots_with_provider(
                vec![dir.path().to_path_buf()],
                Some(provider),
            )
            .await
            .unwrap(),
        );

        // Spawn a slow semantic search in the background.
        let engine_search = Arc::clone(&engine);
        let search_handle = tokio::spawn(async move {
            engine_search
                .execute(CoreOperation::SemanticSearch {
                    query: "hello".to_string(),
                    realm: None,
                    top_k: 5,
                    min_score: 0.0,
                })
                .await
        });

        // Wait until the search task has entered embed() — deterministic sync.
        embed_started.notified().await;

        // Now run a write operation concurrently — it should NOT be blocked
        // by the search because the outer RwLock is released before search.
        let engine_write = Arc::clone(&engine);
        let write_start = Instant::now();
        let write_result = engine_write
            .execute(CoreOperation::CreateRealm {
                name: "write-test".to_string(),
            })
            .await;
        let write_elapsed = write_start.elapsed();

        // The write operation should complete in well under the search delay.
        // If the old lock-contention bug exists, this would take >=150ms.
        assert!(
            write_elapsed < Duration::from_millis(100),
            "CreateRealm took {write_elapsed:?}, expected <100ms — outer read lock may still be held across search",
        );

        assert!(
            matches!(write_result, CoreOperationResult::RealmInfo { .. }),
            "CreateRealm should succeed, got {write_result:?}"
        );

        // Let the search complete and verify it worked.
        let search_result = search_handle.await.expect("search task should not panic");
        assert!(
            matches!(search_result, CoreOperationResult::SemanticMatches(_)),
            "SemanticSearch should succeed, got {search_result:?}"
        );
    }

    /// AddRoot with a slow embedding provider must not block concurrent write
    /// operations on the realm state.
    ///
    /// This test:
    /// 1. Creates an engine with a slow embedding provider (200ms per embed).
    /// 2. Creates a realm, then spawns an AddRoot for a dir with markdown files.
    /// 3. Waits until the embedding provider has started (deterministic sync).
    /// 4. Concurrently runs a CreateRealm write operation.
    /// 5. Asserts CreateRealm completes quickly (well under the embedding delay).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn add_root_does_not_block_realm_writes() {
        let delay = Duration::from_millis(200);
        let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

        // Build an engine with the slow provider and a "default" realm (no roots yet).
        let engine = Arc::new(RuntimeEngine {
            state: tokio::sync::RwLock::new({
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "default".to_string(),
                    RealmData::new(Some(Arc::clone(&provider))),
                );
                map
            }),
            provider: Some(provider),
        });

        // Create a temp dir with a markdown file that has a heading (triggers embedding).
        let dir = make_temp_realm_dir("add-root-concurrency");
        fs::write(
            dir.path().join("doc.md"),
            "# Slow Embedding Test\n\nSome content.\n",
        )
        .unwrap();

        // Spawn AddRoot in the background (will be slow due to embedding).
        let engine_add = Arc::clone(&engine);
        let root_path = dir.path().to_path_buf();
        let add_root_handle = tokio::spawn(async move {
            engine_add
                .execute(CoreOperation::AddRoot {
                    realm: "default".to_string(),
                    root: root_path,
                })
                .await
        });

        // Wait until the embedding provider has been called — deterministic sync.
        embed_started.notified().await;

        // Now run a write operation concurrently — it should NOT be blocked.
        let engine_write = Arc::clone(&engine);
        let write_start = Instant::now();
        let write_result = engine_write
            .execute(CoreOperation::CreateRealm {
                name: "add-root-write-test".to_string(),
            })
            .await;
        let write_elapsed = write_start.elapsed();

        // The write operation should complete in well under the embedding delay.
        // If the write lock is held across indexing, this would take >=200ms.
        assert!(
            write_elapsed < Duration::from_millis(100),
            "CreateRealm took {write_elapsed:?}, expected <100ms — write lock may be held across AddRoot indexing",
        );

        assert!(
            matches!(write_result, CoreOperationResult::RealmInfo { .. }),
            "CreateRealm should succeed, got {write_result:?}"
        );

        // Let AddRoot complete and verify it succeeded.
        let add_root_result = add_root_handle
            .await
            .expect("add_root task should not panic");
        assert!(
            matches!(add_root_result, CoreOperationResult::RealmInfo { .. }),
            "AddRoot should succeed, got {add_root_result:?}"
        );
    }
}

/// Profiles the cumulative I/O cost of N `preview_for_range` calls across
/// N distinct files -- mirrors what semantic search does for top_k results.
///
/// This establishes whether batching/caching previews at the call site
/// (in the SemanticSearch arm of `execute`) would yield meaningful savings.
#[cfg(feature = "semantic-search")]
#[tokio::test]
#[ignore = "performance profiling -- run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_multi_file --ignored --nocapture"]
async fn preview_io_cost_multi_file() {
    use markymark_core::Position;
    use std::time::Instant;

    let dir = make_temp_realm_dir("preview-io-multi");
    const FILE_BYTES: usize = 500_000; // 500 KB per file
    const ITERS: u32 = 20;

    eprintln!(
        "\n{:<8} {:<12} {:<16} {:<16} {:<16}",
        "n_files", "total_bytes", "full_total_avg", "stream_total_avg", "savings"
    );
    eprintln!("{}", "-".repeat(72));

    for &n_files in &[1usize, 5, 10, 20] {
        let mut uris = Vec::new();
        let mut paths = Vec::new();
        let mut target_lines = Vec::new();

        for i in 0..n_files {
            let content = generate_preview_corpus(FILE_BYTES);
            let line_count = content.lines().count() as u32;
            let path = dir.path().join(format!("multi_{n_files}_file_{i}.md"));
            fs::write(&path, &content).unwrap();
            uris.push(DocumentUri::from_file_path(&path));
            target_lines.push(line_count * 3 / 4);
            paths.push(path);
        }

        // Warm up.
        for (uri, &tl) in uris.iter().zip(target_lines.iter()) {
            let range = Range {
                start: Position {
                    line: tl,
                    character: 0,
                },
                end: Position {
                    line: tl + 6,
                    character: 0,
                },
            };
            let _ = helpers::preview_for_range(uri, range, "fallback");
        }

        // Measure: full-read approach across all files.
        let t0 = Instant::now();
        for _ in 0..ITERS {
            for (uri, &tl) in uris.iter().zip(target_lines.iter()) {
                let range = Range {
                    start: Position {
                        line: tl,
                        character: 0,
                    },
                    end: Position {
                        line: tl + 6,
                        character: 0,
                    },
                };
                let _ = helpers::preview_for_range(uri, range, "fallback");
            }
        }
        let full_avg = t0.elapsed() / ITERS;

        // Measure: streaming approach across all files.
        let t1 = Instant::now();
        for _ in 0..ITERS {
            for (path, &tl) in paths.iter().zip(target_lines.iter()) {
                let _ = streamed_preview(path, tl, 200);
            }
        }
        let stream_avg = t1.elapsed() / ITERS;

        let savings_us = full_avg.as_micros().saturating_sub(stream_avg.as_micros());
        eprintln!(
            "{:<8} {:<12} {:<16?} {:<16?} {:<}us saved",
            n_files,
            n_files * FILE_BYTES,
            full_avg,
            stream_avg,
            savings_us,
        );
    }
}
