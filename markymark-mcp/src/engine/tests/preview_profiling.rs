use super::*;
use markymark_core::{Position, Range};
use std::fs;
use std::time::Instant;

/// Generate a synthetic markdown corpus of approximately `target_bytes`.
/// Each section is ~130 bytes so 1 MB ~ 7 600 sections ~ 45 600 lines.
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
#[tokio::test]
#[ignore = "performance profiling -- run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_large_file --ignored --nocapture"]
async fn preview_io_cost_large_file() {
    let dir = make_temp_realm_dir();
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

/// Profiles the cumulative I/O cost of N `preview_for_range` calls across
/// N distinct files -- mirrors what semantic search does for top_k results.
///
/// This establishes whether batching/caching previews at the call site
/// (in the SemanticSearch arm of `execute`) would yield meaningful savings.
#[tokio::test]
#[ignore = "performance profiling -- run manually: cargo test -p markymark-mcp --features semantic-search -- preview_io_cost_multi_file --ignored --nocapture"]
async fn preview_io_cost_multi_file() {
    let dir = make_temp_realm_dir();
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
