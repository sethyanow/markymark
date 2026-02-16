//! Benchmarks for incremental vs full reparse performance.
//!
//! Measures two levels:
//! 1. Tree-sitter parse only (validates incremental tree reuse)
//! 2. Full pipeline: parse + AST construction (current end-to-end cost)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use markymark_parser::{byte_to_point, InputEdit, Parser};
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

criterion_group!(
    benches,
    bench_scaling,
    bench_full_pipeline,
    print_speedup_summary
);
criterion_main!(benches);
