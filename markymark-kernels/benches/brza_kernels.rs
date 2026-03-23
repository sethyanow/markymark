//! BRZA benchmark suite for SIMD kernels vs baseline implementations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use markymark_core::scanner::{Md4cScanBackend, ScanBackend, ZigScanBackend};
use markymark_index::DocumentIndex;
use markymark_kernels::{embed::EmbeddingIndex, scan, tokens};
use markymark_parser::Parser;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

const EMBED_DIMS: u32 = 32;
const BULK_DOC_TARGET: usize = 600;
const EXCLUDED_DIRS: &[&str] = &[".git", "target", "node_modules"];

struct SearchFixture {
    index: EmbeddingIndex,
    query: Vec<f32>,
}

fn sample_size(default: usize) -> usize {
    std::env::var("MARKYMARK_BENCH_SAMPLE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(10, 100)
}

fn generate_markdown_doc(target_bytes: usize) -> String {
    let mut doc = String::with_capacity(target_bytes + 512);
    let mut section = 1usize;
    doc.push_str("# BRZA benchmark corpus\n\n");
    while doc.len() < target_bytes {
        doc.push_str(&format!("## Section {section}\n\n"));
        doc.push_str("Fast paths should skip fenced code and still catch [[wiki links]].\n");
        doc.push_str("- Item with [markdown](https://example.com/a)\n");
        doc.push_str("- Item with [[Wiki Target|Alias]] and #tag\n");
        doc.push_str("Paragraph with ^block-id and [docs](https://example.com/docs).\n\n");
        doc.push_str("```rust\n");
        doc.push_str("fn ignored() { let x = \"[[not_a_link]]\"; }\n");
        doc.push_str("```\n\n");
        section += 1;
    }
    doc
}

fn generate_link_heavy_doc(link_pairs: usize) -> String {
    let mut doc = String::with_capacity(link_pairs * 90);
    doc.push_str("# Link-heavy corpus\n\n");
    for i in 0..link_pairs {
        doc.push_str(&format!(
            "- [Doc {i}](https://example.com/{i}) [[Page {i}|Alias {i}]] #tag{i}\n"
        ));
    }
    doc
}

fn count_tree_sitter_headings(doc: &str, parser: &mut Parser) -> usize {
    match parser.parse(doc) {
        Ok(ast) => ast
            .root_elements()
            .iter()
            .filter(|e| e.as_heading().is_some())
            .count(),
        Err(err) => {
            eprintln!("  [brza] tree-sitter parse failed in heading benchmark: {err}");
            0
        }
    }
}

fn count_regex_links(doc: &str, markdown: &Regex, wiki: &Regex) -> usize {
    markdown.find_iter(doc).count() + wiki.find_iter(doc).count()
}

fn embedding_for(id: usize, dims: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(dims);
    for j in 0..dims {
        let v = (((id * 131) ^ (j * 17) ^ (id >> 3)) % 10_000) as f32 / 10_000.0;
        out.push(v);
    }
    out
}

fn build_embedding_fixture(entry_count: usize) -> SearchFixture {
    let mut index =
        EmbeddingIndex::new(EMBED_DIMS).expect("embedding index initialization should succeed");
    let mut embedding = vec![0.0f32; EMBED_DIMS as usize];
    for i in 0..entry_count {
        embedding[0] = (i % 1024) as f32 / 1024.0;
        embedding[1] = ((i / 7) % 1024) as f32 / 1024.0;
        embedding[2] = ((i / 13) % 1024) as f32 / 1024.0;
        let id = format!("doc-{i}");
        index
            .add(&id, &embedding)
            .expect("embedding insert should succeed");
    }
    let query = embedding_for(entry_count / 2, EMBED_DIMS as usize);
    SearchFixture { index, query }
}

fn generate_symbol_candidates(count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        if i % 97 == 0 {
            out.push(format!("state-machine-{i}"));
        } else if i % 53 == 0 {
            out.push(format!("stack-trace-{i}"));
        } else {
            out.push(format!("candidate-symbol-{i:06}"));
        }
    }
    out
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
    if out.len() >= max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if EXCLUDED_DIRS.contains(&name) {
                continue;
            }
            collect_markdown_files(&path, out, max_files);
            if out.len() >= max_files {
                return;
            }
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
            if out.len() >= max_files {
                return;
            }
        }
    }
}

fn load_bulk_docs(max_docs: usize) -> Vec<String> {
    let mut files = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("workspace root");

    collect_markdown_files(&root.join("docs"), &mut files, max_docs);
    collect_markdown_files(root, &mut files, max_docs);

    let mut docs = files
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter(|content| !content.trim().is_empty())
        .collect::<Vec<_>>();

    if docs.is_empty() {
        return docs;
    }

    // Expand to the requested corpus size by cycling real docs.
    let mut i = 0usize;
    while docs.len() < max_docs {
        docs.push(docs[i % docs.len()].clone());
        i += 1;
    }
    docs.truncate(max_docs);
    docs
}

fn bench_heading_scan_vs_tree_sitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("heading_scan_vs_tree_sitter");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    let mut parser = Parser::new().expect("parser initialization should succeed");
    for (label, bytes) in [("1kb", 1_024usize), ("10kb", 10_240), ("100kb", 102_400)] {
        let doc = generate_markdown_doc(bytes);
        group.sample_size(if bytes >= 100_000 {
            sample_size(12)
        } else {
            sample_size(20)
        });
        group.throughput(Throughput::Bytes(doc.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("zig_heading_scan", label),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let count = scan::scan_headings(black_box(doc))
                        .map(|v| v.len())
                        .unwrap_or_default();
                    black_box(count)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("tree_sitter_headings", label),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let count = count_tree_sitter_headings(black_box(doc), &mut parser);
                    black_box(count)
                });
            },
        );
    }

    group.finish();
}

fn bench_link_scan_vs_regex(c: &mut Criterion) {
    let mut group = c.benchmark_group("link_scan_vs_regex");
    group.sample_size(sample_size(20));
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    let markdown_re =
        Regex::new(r"\[[^\]\n]+\]\([^)]+\)").expect("markdown link regex should compile");
    let wiki_re = Regex::new(r"\[\[[^\]\n]+\]\]").expect("wiki link regex should compile");

    for (label, link_pairs) in [
        ("100_links", 100usize),
        ("500_links", 500),
        ("2k_links", 2_000),
    ] {
        let doc = generate_link_heavy_doc(link_pairs);
        group.throughput(Throughput::Elements(link_pairs as u64));

        group.bench_with_input(BenchmarkId::new("zig_link_scan", label), &doc, |b, doc| {
            b.iter(|| {
                let count = scan::scan_links(black_box(doc))
                    .map(|v| v.len())
                    .unwrap_or_default();
                black_box(count)
            });
        });

        group.bench_with_input(BenchmarkId::new("regex_links", label), &doc, |b, doc| {
            b.iter(|| {
                let count = count_regex_links(black_box(doc), &markdown_re, &wiki_re);
                black_box(count)
            });
        });
    }

    group.finish();
}

fn bench_embedding_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_search");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for &entries in &[1_000usize, 10_000, 100_000] {
        group.sample_size(if entries >= 100_000 {
            sample_size(10)
        } else {
            sample_size(20)
        });
        group.throughput(Throughput::Elements(entries as u64));
        let fixture = OnceLock::new();
        group.bench_function(
            BenchmarkId::new("search_top10", format!("{entries}_entries")),
            |b| {
                let fixture = fixture.get_or_init(|| {
                    eprintln!("  [brza] building embedding fixture ({entries} entries)");
                    build_embedding_fixture(entries)
                });
                b.iter(|| {
                    let hits = fixture
                        .index
                        .search(black_box(&fixture.query), 10)
                        .map(|v| v.len())
                        .unwrap_or_default();
                    black_box(hits)
                });
            },
        );
    }

    group.finish();
}

fn bench_content_hash_vs_md5(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_hash_vs_md5");
    group.sample_size(sample_size(20));
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    for (label, bytes) in [("1kb", 1_024usize), ("10kb", 10_240), ("100kb", 102_400)] {
        let doc = generate_markdown_doc(bytes);
        group.throughput(Throughput::Bytes(doc.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("zig_content_hash", label),
            &doc,
            |b, doc| {
                b.iter(|| black_box(tokens::content_hash(black_box(doc))));
            },
        );

        group.bench_with_input(BenchmarkId::new("md5_hash", label), &doc, |b, doc| {
            b.iter(|| black_box(md5::compute(black_box(doc.as_bytes()))));
        });
    }

    group.finish();
}

fn bench_fuzzy_match_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("fuzzy_match_batch");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    const QUERY: &str = "sta";

    for (label, entries, top_k) in [
        ("10k_candidates", 10_000usize, 25usize),
        ("100k_candidates", 100_000usize, 25usize),
    ] {
        group.sample_size(if entries >= 100_000 {
            sample_size(10)
        } else {
            sample_size(20)
        });
        group.throughput(Throughput::Elements(entries as u64));

        let fixture = OnceLock::new();
        group.bench_function(BenchmarkId::new("batch_topk", label), |b| {
            let symbols = fixture.get_or_init(|| generate_symbol_candidates(entries));
            let refs: Vec<&str> = symbols.iter().map(String::as_str).collect();

            b.iter(|| {
                let hits = scan::fuzzy_match_batch(black_box(QUERY), black_box(&refs), top_k)
                    .map(|v| v.len())
                    .unwrap_or_default();
                black_box(hits)
            });
        });

        let fixture = OnceLock::new();
        group.bench_function(BenchmarkId::new("per_candidate_sort", label), |b| {
            let symbols = fixture.get_or_init(|| generate_symbol_candidates(entries));
            let refs: Vec<&str> = symbols.iter().map(String::as_str).collect();

            b.iter(|| {
                let mut scored = Vec::with_capacity(refs.len());
                for (idx, candidate) in refs.iter().enumerate() {
                    if let Ok(m) = scan::fuzzy_match(QUERY, candidate) {
                        if m.score > 0 {
                            scored.push((m.score, idx));
                        }
                    }
                }

                scored.sort_by(|(score_a, idx_a), (score_b, idx_b)| {
                    score_b.cmp(score_a).then_with(|| idx_a.cmp(idx_b))
                });

                black_box(scored.len().min(top_k))
            });
        });
    }

    group.finish();
}

fn bench_bulk_reindex(c: &mut Criterion) {
    let docs = load_bulk_docs(BULK_DOC_TARGET);
    if docs.is_empty() {
        eprintln!("  [brza] Skipping bulk_reindex benchmark: no markdown corpus found");
        return;
    }

    let mut group = c.benchmark_group("bulk_reindex");
    group.sample_size(sample_size(10));
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(6));
    group.throughput(Throughput::Elements(docs.len() as u64));

    let backend = ZigScanBackend;
    group.bench_function("zig_scan_backend_600_docs", |b| {
        b.iter(|| {
            let mut heading_total = 0usize;
            for doc in &docs {
                let idx = DocumentIndex::from_scan(black_box(doc), &backend);
                heading_total += idx.headings().len();
            }
            black_box(heading_total)
        });
    });

    group.bench_function("engine_from_text_600_docs", |b| {
        b.iter(|| {
            let mut heading_total = 0usize;
            for doc in &docs {
                let idx = DocumentIndex::from_text(black_box(doc));
                heading_total += idx.headings().len();
            }
            black_box(heading_total)
        });
    });

    group.finish();
}

fn bench_md4c_vs_tree_sitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("md4c_vs_tree_sitter");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    let mut parser = Parser::new().expect("parser initialization should succeed");
    let md4c_backend = Md4cScanBackend;

    // One-time correctness assertion: md4c heading count must match tree-sitter
    // on the same document (runs before benchmarks, not in the hot path).
    // Both sides use fail-fast error handling so a silent 0 can't mask a parse failure.
    {
        let check_doc = generate_markdown_doc(10_240);
        let ts_count = match parser.parse(&check_doc) {
            Ok(ast) => ast
                .root_elements()
                .iter()
                .filter(|e| e.as_heading().is_some())
                .count(),
            Err(err) => panic!("tree-sitter parity check failed: {err}"),
        };
        let md4c_count = md4c_backend
            .scan_headings(&check_doc)
            .expect("md4c parity check failed")
            .len();
        assert_eq!(
            ts_count, md4c_count,
            "md4c heading count ({md4c_count}) != tree-sitter heading count ({ts_count}) on 10KB doc"
        );
    }

    for (label, bytes) in [
        ("1kb", 1_024usize),
        ("10kb", 10_240),
        ("50kb", 51_200),
        ("100kb", 102_400),
    ] {
        let doc = generate_markdown_doc(bytes);
        group.sample_size(if bytes >= 50_000 {
            sample_size(12)
        } else {
            sample_size(20)
        });
        group.throughput(Throughput::Bytes(doc.len() as u64));

        // md4c scan backend → from_scan (full index build via FFI)
        group.bench_with_input(BenchmarkId::new("md4c_from_scan", label), &doc, |b, doc| {
            b.iter(|| {
                let idx = DocumentIndex::from_scan(black_box(doc), &md4c_backend);
                black_box(idx.headings().len())
            });
        });

        // engine → from_text (full index build via ephemeral engine)
        group.bench_with_input(
            BenchmarkId::new("engine_from_text", label),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let idx = DocumentIndex::from_text(black_box(doc));
                    black_box(idx.headings().len())
                });
            },
        );

        // md4c raw FFI extraction only (no index build, just parse + extract)
        group.bench_with_input(
            BenchmarkId::new("md4c_extract_only", label),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let extraction =
                        markymark_kernels::md4c::extract_md4c(black_box(doc)).expect("md4c");
                    black_box(extraction.headings.len() + extraction.links.len())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_heading_scan_vs_tree_sitter,
    bench_link_scan_vs_regex,
    bench_embedding_search,
    bench_content_hash_vs_md5,
    bench_fuzzy_match_batch,
    bench_bulk_reindex,
    bench_md4c_vs_tree_sitter
);
criterion_main!(benches);
