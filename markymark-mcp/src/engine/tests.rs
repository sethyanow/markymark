use super::*;
use markymark_core::Position;
use std::fs;

fn make_temp_realm_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marky-realm-{}-{}", suffix, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_engine_with_custom_realm(realm_name: &str, dir: &Path) -> RuntimeEngine {
    let engine = RuntimeEngine::default();
    // create the realm
    engine.execute(CoreOperation::CreateRealm {
        name: realm_name.to_string(),
    });
    // index the directory into it
    engine.execute(CoreOperation::AddRoot {
        realm: realm_name.to_string(),
        root: dir.to_path_buf(),
    });
    engine
}

#[test]
fn get_outline_uses_named_realm() {
    let dir = make_temp_realm_dir("get-outline");
    fs::write(dir.join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
    let engine = make_engine_with_custom_realm("my-realm", &dir);

    let uri_str = format!("file://{}", dir.join("doc.md").display());
    let uri = DocumentUri::new(&uri_str).unwrap();

    // Should fail without realm (default realm has no such doc)
    let result = engine.execute(CoreOperation::GetOutline {
        uri: uri.clone(),
        realm: None,
    });
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error when querying default realm, got {result:?}"
    );

    // Should succeed with the correct realm
    let result = engine.execute(CoreOperation::GetOutline {
        uri: uri.clone(),
        realm: Some("my-realm".to_string()),
    });
    assert!(
        matches!(result, CoreOperationResult::Outline(_)),
        "expected Outline from named realm, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn export_index_uses_named_realm() {
    let dir = make_temp_realm_dir("export-index");
    fs::write(dir.join("doc.md"), "# Title\n").unwrap();
    let engine = make_engine_with_custom_realm("export-realm", &dir);

    let uri_str = format!("file://{}", dir.join("doc.md").display());
    let uri = DocumentUri::new(&uri_str).unwrap();

    let result = engine.execute(CoreOperation::ExportIndex {
        uri: uri.clone(),
        realm: Some("export-realm".to_string()),
    });
    assert!(
        matches!(result, CoreOperationResult::DocumentExport { .. }),
        "expected DocumentExport from named realm, got {result:?}"
    );

    let result_default = engine.execute(CoreOperation::ExportIndex { uri, realm: None });
    assert!(
        matches!(result_default, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result_default:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_symbols_uses_named_realm() {
    let dir = make_temp_realm_dir("search-symbols");
    fs::write(dir.join("doc.md"), "# UniqueHeadingXYZ\n").unwrap();
    let engine = make_engine_with_custom_realm("search-realm", &dir);

    // Default realm should return no matches for the unique heading
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "UniqueHeadingXYZ".to_string(),
        realm: None,
    });
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(
            matches.is_empty(),
            "default realm should not have the heading"
        );
    } else {
        panic!("expected Symbols result");
    }

    // Named realm should find it
    let result = engine.execute(CoreOperation::SearchSymbols {
        query: "UniqueHeadingXYZ".to_string(),
        realm: Some("search-realm".to_string()),
    });
    if let CoreOperationResult::Symbols(matches) = result {
        assert!(!matches.is_empty(), "named realm should have the heading");
    } else {
        panic!("expected Symbols result");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_references_uses_named_realm() {
    let dir = make_temp_realm_dir("find-refs");
    // A heading with a wiki-link reference in the same file
    fs::write(dir.join("doc.md"), "# My Heading\n\n[[My Heading]]\n").unwrap();
    let engine = make_engine_with_custom_realm("refs-realm", &dir);

    let uri_str = format!("file://{}", dir.join("doc.md").display());
    let uri = DocumentUri::new(&uri_str).unwrap();

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
    let result = engine.execute(CoreOperation::FindReferences {
        uri: uri.clone(),
        position,
        realm: None,
    });
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should find the references
    let result = engine.execute(CoreOperation::FindReferences {
        uri,
        position,
        realm: Some("refs-realm".to_string()),
    });
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rename_uses_named_realm() {
    let dir = make_temp_realm_dir("rename");
    fs::write(dir.join("doc.md"), "# Old Name\n").unwrap();
    let engine = make_engine_with_custom_realm("rename-realm", &dir);

    let uri_str = format!("file://{}", dir.join("doc.md").display());
    let uri = DocumentUri::new(&uri_str).unwrap();

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
    let result = engine.execute(CoreOperation::Rename {
        uri: uri.clone(),
        position,
        new_name: "New Name".to_string(),
        realm: None,
    });
    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected error from default realm, got {result:?}"
    );

    // Named realm should work
    let result = engine.execute(CoreOperation::Rename {
        uri,
        position,
        new_name: "New Name".to_string(),
        realm: Some("rename-realm".to_string()),
    });
    assert!(
        !matches!(result, CoreOperationResult::Error(_)),
        "expected success from named realm, got {result:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn collect_documents_includes_json_alongside_markdown() {
    let dir = std::env::temp_dir().join(format!("marky-collect-mixed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("notes.md"), "# Hello\n").unwrap();
    fs::write(dir.join("config.json"), "{}").unwrap();
    fs::write(dir.join("settings.yaml"), "key: val\n").unwrap();
    fs::write(dir.join("main.rs"), "fn main() {}").unwrap();

    let docs = helpers::collect_documents(&dir);
    let kinds: Vec<_> = docs.iter().map(|(_, k)| *k).collect();

    assert!(kinds.contains(&DocumentKind::Markdown));
    assert!(kinds.contains(&DocumentKind::Json));
    assert!(kinds.contains(&DocumentKind::Yaml));
    // main.rs should NOT be collected
    assert_eq!(docs.len(), 3);

    let _ = fs::remove_dir_all(&dir);
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
#[test]
fn fnv1a32_is_stable_and_deterministic() {
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
    // Empty string → offset basis unchanged
    assert_eq!(
        fnv1a32(b""),
        0x811c9dc5,
        "empty bytes must return FNV offset basis"
    );
}

/// HashEmbeddingProvider must produce a normalized output vector of the
/// expected dimensionality.
#[cfg(feature = "semantic-search")]
#[test]
fn hash_embedding_output_is_normalized_and_correct_dims() {
    let provider = HashEmbeddingProvider::new(128);
    let emb = provider.embed("hello world").unwrap();
    assert_eq!(emb.len(), 128, "embedding length must match dims");
    let norm_sq: f32 = emb.iter().map(|v| v * v).sum();
    assert!(
        (norm_sq - 1.0).abs() < 1e-5,
        "embedding must be L2-normalised, got norm²={norm_sq}"
    );
}

/// HashEmbeddingProvider must produce identical vectors for identical input.
/// This test detects accidental use of randomised hashing (e.g. RandomState).
#[cfg(feature = "semantic-search")]
#[test]
fn hash_embedding_is_deterministic() {
    let provider = HashEmbeddingProvider::new(64);
    let a = provider.embed("markymark semantic search").unwrap();
    let b = provider.embed("markymark semantic search").unwrap();
    assert_eq!(a, b, "identical input must produce identical embedding");
}

/// Empty text must fail with InvalidInput (not panic).
#[cfg(feature = "semantic-search")]
#[test]
fn hash_embedding_rejects_empty_text() {
    let provider = HashEmbeddingProvider::new(32);
    let err = provider.embed("   ").unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "whitespace-only input must return InvalidInput, got {err:?}"
    );
}

/// Zero dims must fail with InvalidInput (not divide-by-zero).
#[cfg(feature = "semantic-search")]
#[test]
fn hash_embedding_rejects_zero_dims() {
    let provider = HashEmbeddingProvider::new(0);
    let err = provider.embed("hello").unwrap_err();
    assert!(
        matches!(err, markymark_core::prelude::EmbedError::InvalidInput(_)),
        "zero dims must return InvalidInput, got {err:?}"
    );
}

#[test]
fn collect_documents_markdown_unchanged() {
    let dir = std::env::temp_dir().join(format!("marky-collect-md-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("readme.md"), "# R\n").unwrap();
    fs::write(dir.join("guide.markdown"), "# G\n").unwrap();

    let docs = helpers::collect_documents(&dir);
    assert_eq!(docs.len(), 2);
    assert!(docs.iter().all(|(_, k)| *k == DocumentKind::Markdown));

    let _ = fs::remove_dir_all(&dir);
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
/// Each section is ~130 bytes so 1 MB ≈ 7 600 sections ≈ 45 600 lines.
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

/// Alternative preview extraction using `BufRead::lines()` — reads only
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
#[test]
#[ignore = "performance profiling — run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_large_file --ignored --nocapture"]
fn preview_io_cost_large_file() {
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
        let path = dir.join(format!("doc_{target_bytes}.md"));
        fs::write(&path, &content).unwrap();

        let uri_str = format!("file://{}", path.display());
        let uri = DocumentUri::new(&uri_str).unwrap();

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

    let _ = fs::remove_dir_all(&dir);
}

/// Profiles the cumulative I/O cost of N `preview_for_range` calls across
/// N distinct files — mirrors what semantic search does for top_k results.
///
/// This establishes whether batching/caching previews at the call site
/// (in the SemanticSearch arm of `execute`) would yield meaningful savings.
#[cfg(feature = "semantic-search")]
#[test]
#[ignore = "performance profiling — run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_multi_file --ignored --nocapture"]
fn preview_io_cost_multi_file() {
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
            let path = dir.join(format!("multi_{n_files}_file_{i}.md"));
            fs::write(&path, &content).unwrap();
            let uri_str = format!("file://{}", path.display());
            uris.push(DocumentUri::new(&uri_str).unwrap());
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
            "{:<8} {:<12} {:<16?} {:<16?} {:<}µs saved",
            n_files,
            n_files * FILE_BYTES,
            full_avg,
            stream_avg,
            savings_us,
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
