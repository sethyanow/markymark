//! Benchmarks for incremental vs full reparse performance.
//!
//! Measures two levels:
//! 1. Tree-sitter parse only (validates incremental tree reuse)
//! 2. Full pipeline: parse + AST construction (current end-to-end cost)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use markymark_parser::{byte_to_point, find_prose_edit_pos, InputEdit, Parser};
use std::path::{Path, PathBuf};
use tree_sitter_md::MarkdownParser;

/// Generate a markdown document of approximately `target_bytes` with realistic structure.
fn generate_large_doc(target_bytes: usize) -> String {
    let mut doc = String::with_capacity(target_bytes + 1024);
    let mut section = 1;

    doc.push_str("# Large Document\n\n");
    doc.push_str("This is the introduction paragraph with some text.\n\n");

    while doc.len() < target_bytes {
        doc.push_str(&format!("## Section {section}\n\n"));
        for para in 0..3 {
            doc.push_str(&format!(
                "Paragraph {para} in section {section}. \
                 Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                 Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
                 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.\n\n"
            ));
        }
        doc.push_str("- Item one with [[wiki link]]\n");
        doc.push_str("- Item two with `inline code`\n");
        doc.push_str("- Item three with #tag\n\n");
        doc.push_str("```rust\nfn example() {\n    println!(\"hello\");\n}\n```\n\n");
        section += 1;
    }

    doc
}

/// Prepare an edited source and tree for incremental parsing benchmarks.
fn prepare_incremental_edit(
    doc: &str,
    md_tree: &tree_sitter_md::MarkdownTree,
) -> (String, tree_sitter_md::MarkdownTree) {
    let insert_pos = doc.len() / 2;
    let mut source = doc.to_string();
    let mut tree = md_tree.clone();

    let start_position = byte_to_point(doc, insert_pos);
    let new_end_byte = insert_pos + 1;
    source.insert(insert_pos, 'x');
    let new_end_position = byte_to_point(&source, new_end_byte);

    tree.edit(&InputEdit {
        start_byte: insert_pos,
        old_end_byte: insert_pos,
        new_end_byte,
        start_position,
        old_end_position: start_position,
        new_end_position,
    });

    (source, tree)
}

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.sample_size(50);

    for &size_kb in &[10, 50, 200] {
        let doc = generate_large_doc(size_kb * 1024);
        let mut raw_parser = MarkdownParser::default();
        let md_tree = raw_parser.parse(doc.as_bytes(), None).unwrap();

        group.bench_with_input(
            BenchmarkId::new("ts_full", format!("{size_kb}kb")),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let _tree = raw_parser.parse(black_box(doc.as_bytes()), None).unwrap();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ts_incremental", format!("{size_kb}kb")),
            &doc,
            |b, doc| {
                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let (source, tree) = prepare_incremental_edit(doc, &md_tree);
                        let start = std::time::Instant::now();
                        let _new_tree = raw_parser
                            .parse(black_box(source.as_bytes()), Some(black_box(&tree)))
                            .unwrap();
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_50kb");
    let doc = generate_large_doc(50_000);

    let mut parser = Parser::new().unwrap();
    let mut ast = parser.parse(&doc).unwrap();
    let md_tree = ast.take_md_tree().unwrap();

    group.bench_function("full", |b| {
        b.iter(|| {
            let _ast = parser.parse(black_box(&doc)).unwrap();
        });
    });

    group.bench_function("incremental", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let (source, tree) = prepare_incremental_edit(&doc, &md_tree);
                let start = std::time::Instant::now();
                let _ast = parser
                    .parse_with_old_tree(black_box(&source), Some(black_box(&tree)))
                    .unwrap();
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

fn print_speedup_summary(c: &mut Criterion) {
    let _ = c;
    let iterations = 200;

    eprintln!("\n=== INCREMENTAL PARSE SPEEDUP SUMMARY ===\n");
    eprintln!(
        "{:>8}  {:>12}  {:>12}  {:>8}",
        "Size", "Full (us)", "Incr (us)", "Speedup"
    );
    eprintln!("{:->8}  {:->12}  {:->12}  {:->8}", "", "", "", "");

    for &size_kb in &[10, 50, 200] {
        let doc = generate_large_doc(size_kb * 1024);
        let mut raw_parser = MarkdownParser::default();
        let md_tree = raw_parser.parse(doc.as_bytes(), None).unwrap();

        let mut full_time = std::time::Duration::ZERO;
        let mut inc_time = std::time::Duration::ZERO;

        for _ in 0..iterations {
            let start = std::time::Instant::now();
            let _ = raw_parser.parse(doc.as_bytes(), None).unwrap();
            full_time += start.elapsed();

            let (source, tree) = prepare_incremental_edit(&doc, &md_tree);
            let start = std::time::Instant::now();
            let _ = raw_parser.parse(source.as_bytes(), Some(&tree)).unwrap();
            inc_time += start.elapsed();
        }

        let ratio = full_time.as_nanos() as f64 / inc_time.as_nanos() as f64;
        let full_avg = full_time.as_nanos() as f64 / iterations as f64 / 1000.0;
        let inc_avg = inc_time.as_nanos() as f64 / iterations as f64 / 1000.0;

        eprintln!("{size_kb:>6}kb  {full_avg:>12.1}  {inc_avg:>12.1}  {ratio:>7.1}x");
    }

    eprintln!("\nNote: tree-sitter-md uses dual block+inline grammars.");
    eprintln!("Inline reparsing limits incremental speedup at the parse level.");
    eprintln!("Phase 3 (incremental indexing) will skip AST/index rebuild");
    eprintln!("for unchanged sections, providing the 10x+ end-to-end speedup.\n");
}

// ---------------------------------------------------------------------------
// Corpus benchmark helpers
// ---------------------------------------------------------------------------

/// Path to the markdown corpus directory.
///
/// Checks `MARKYMARK_BENCH_CORPUS_DIR`, then falls back to `/Volumes/code/gigapowers`.
fn docs_corpus_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("MARKYMARK_BENCH_CORPUS_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }
    let default = PathBuf::from("/Volumes/code/gigapowers");
    default.is_dir().then_some(default)
}

/// Collect one representative `.md` file per size bucket from `dir`.
///
/// Buckets: ~50KB, ~100KB, ~400KB. Files with no qualifying prose edit position are skipped.
/// Returns `(label, content)` — label is size-based only, never the real file path.
fn collect_corpus_samples(dir: &Path) -> Vec<(String, String)> {
    const EXCLUDE_DIRS: &[&str] = &["node_modules", ".git"];
    const BUCKETS: &[(&str, usize, usize, usize)] = &[
        ("~50KB", 50_000, 30_000, 70_000),
        ("~100KB", 100_000, 75_000, 150_000),
        ("~400KB", 400_000, 300_000, 600_000),
    ];

    fn walk(dir: &Path, out: &mut Vec<(u64, PathBuf)>, excludes: &[&str]) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !excludes.contains(&name) {
                    walk(&path, out, excludes);
                }
            } else if path.extension().is_some_and(|e| e == "md") {
                if let Ok(meta) = path.metadata() {
                    out.push((meta.len(), path));
                }
            }
        }
    }

    let mut all: Vec<(u64, PathBuf)> = Vec::new();
    walk(dir, &mut all, EXCLUDE_DIRS);
    all.sort_by_key(|(sz, _)| *sz);

    let mut results = Vec::new();
    for &(label, target, min, max) in BUCKETS {
        let best = all
            .iter()
            .filter(|(sz, _)| *sz as usize >= min && *sz as usize <= max)
            .min_by_key(|(sz, _)| (*sz as usize).abs_diff(target));

        if let Some((sz, path)) = best {
            if let Ok(content) = std::fs::read_to_string(path) {
                if find_prose_edit_pos(&content).is_some() {
                    let size_kb = sz / 1024;
                    results.push((format!("{label} ({size_kb}KB)"), content));
                }
            }
        }
    }

    results
}

/// Real-corpus incremental parse speedup summary.
///
/// Picks one representative file per size bucket (~50KB, ~100KB, ~400KB) from the corpus
/// directory, applies a single-character prose edit near the document midpoint, and
/// measures tree-sitter full vs. incremental parse time.
///
/// Skips gracefully when no corpus is available.
///
/// Run with:
/// ```
/// cargo bench -p markymark-parser --bench incremental -- print_corpus_speedup_summary
/// ```
fn print_corpus_speedup_summary(c: &mut Criterion) {
    let _ = c;

    let Some(corpus_dir) = docs_corpus_dir() else {
        eprintln!("\n=== CORPUS INCREMENTAL SPEEDUP (skipped: corpus not found) ===");
        eprintln!("Set MARKYMARK_BENCH_CORPUS_DIR or place files at /Volumes/code/gigapowers.\n");
        return;
    };

    let samples = collect_corpus_samples(&corpus_dir);
    if samples.is_empty() {
        eprintln!("\n=== CORPUS INCREMENTAL SPEEDUP (skipped: no qualifying files found) ===\n");
        return;
    }

    let iterations = 100u32;
    eprintln!("\n=== CORPUS INCREMENTAL SPEEDUP SUMMARY ===\n");
    eprintln!(
        "{:<18}  {:>12}  {:>12}  {:>8}",
        "File", "Full (us)", "Incr (us)", "Speedup"
    );
    eprintln!("{:-<18}  {:->12}  {:->12}  {:->8}", "", "", "", "");

    for (label, content) in &samples {
        let edit_pos = match find_prose_edit_pos(content) {
            Some(p) => p,
            None => continue,
        };

        let mut raw_parser = MarkdownParser::default();
        let md_tree = raw_parser.parse(content.as_bytes(), None).unwrap();

        // Precompute the edited source once (string insert is O(n) and not part of parse timing)
        let mut edited = content.clone();
        edited.insert(edit_pos, 'x');
        let start_position = byte_to_point(content, edit_pos);
        let new_end_byte = edit_pos + 1;
        let new_end_position = byte_to_point(&edited, new_end_byte);

        let mut full_time = std::time::Duration::ZERO;
        let mut inc_time = std::time::Duration::ZERO;

        for _ in 0..iterations {
            // Full reparse
            let start = std::time::Instant::now();
            let _ = raw_parser.parse(content.as_bytes(), None).unwrap();
            full_time += start.elapsed();

            // Incremental: clone + apply edit outside the timed window
            let mut tree = md_tree.clone();
            tree.edit(&InputEdit {
                start_byte: edit_pos,
                old_end_byte: edit_pos,
                new_end_byte,
                start_position,
                old_end_position: start_position,
                new_end_position,
            });

            let start = std::time::Instant::now();
            let _ = raw_parser.parse(edited.as_bytes(), Some(&tree)).unwrap();
            inc_time += start.elapsed();
        }

        let ratio = full_time.as_nanos() as f64 / inc_time.as_nanos().max(1) as f64;
        let full_avg = full_time.as_nanos() as f64 / iterations as f64 / 1000.0;
        let inc_avg = inc_time.as_nanos() as f64 / iterations as f64 / 1000.0;

        eprintln!("{label:<18}  {full_avg:>12.1}  {inc_avg:>12.1}  {ratio:>7.1}x");
    }

    eprintln!("\nEdit: single-char insert at middle-document prose line (no wiki links).");
    eprintln!("Level: tree-sitter parse only — does not include index rebuild.\n");
}

criterion_group!(
    benches,
    bench_scaling,
    bench_full_pipeline,
    print_speedup_summary,
    print_corpus_speedup_summary
);
criterion_main!(benches);
