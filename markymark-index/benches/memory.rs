//! Memory and performance benchmarks for arena allocation.
//!
//! Measures: indexing N documents (time, peak RSS), allocation count, re-parse time,
//! memory footprint, and concurrent indexing throughput.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

/// Counting allocator: wraps System and increments a counter on each alloc.
/// Note: Bumpalo uses its own allocation, so this counts only non-arena heap allocations
/// (e.g. RealmIndex HashMap entries, DocumentUri strings, cross-doc owned copies).
struct CountingAllocator;

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static A: CountingAllocator = CountingAllocator;

use markymark_core::DocumentUri;
use markymark_index::{
    bench_doc_counts, bench_sample_size, build_mixed_size_corpus, DocumentIndex, RealmIndex,
};
use markymark_parser::Parser;
use std::path::{Path, PathBuf};

/// Directories to exclude when collecting .md files.
const EXCLUDE_DIRS: &[&str] = &["node_modules"];

fn sample_doc(n: usize) -> String {
    format!(
        r#"# Document {}

## Section A
Content with [[wiki link]] and #tag and [markdown](https://example.com) link.

A block ^block-{}

## Section B
More content here.
"#,
        n, n
    )
}

/// Path to epstein fixture. Set MARKYMARK_BENCH_EPSTEIN or use workspace/epstein_20250227_all_in_one.md.
fn real_corpus_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MARKYMARK_BENCH_EPSTEIN") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.ancestors().nth(1)?;
    let path = workspace.join("epstein_20250227_all_in_one.md");
    path.exists().then_some(path)
}

/// Path to docs corpus directory. Set MARKYMARK_BENCH_CORPUS_DIR or use default /Volumes/code/gigapowers.
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

/// Recursively collect .md file paths, excluding EXCLUDE_DIRS. Caps at max_files.
fn collect_md_files(dir: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
        if out.len() >= max_files {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if EXCLUDE_DIRS.contains(&name) {
                    continue;
                }
                walk(&path, out, max_files);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
                if out.len() >= max_files {
                    return;
                }
            }
        }
    }
    walk(dir, &mut out, max_files);
    out
}

fn index_n_documents(n: usize) -> RealmIndex {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut parser = Parser::new().expect("parser init");
    let mut realm = RealmIndex::new();

    for i in 0..n {
        let uri = DocumentUri::from_file_path(&PathBuf::from(format!("/vault/doc{}.md", i)));
        let content = sample_doc(i);
        let ast = parser.parse(&content).expect("parse");
        let index = DocumentIndex::from_ast(ast);
        rt.block_on(realm.add_document(uri, index));
    }

    realm
}

fn reparse_single_document(content: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(content).expect("parse");
    DocumentIndex::from_ast(ast)
}

fn bench_index_10_docs(c: &mut Criterion) {
    c.bench_function("index_10_documents", |b| {
        b.iter(|| {
            let realm = index_n_documents(10);
            black_box(realm.document_count())
        })
    });
}

fn bench_index_100_docs(c: &mut Criterion) {
    c.bench_function("index_100_documents", |b| {
        b.iter(|| {
            let realm = index_n_documents(100);
            black_box(realm.document_count())
        })
    });
}

/// Index mixed-size synthetic corpus at scaling tiers (100 -> 10_000 docs).
fn bench_index_synthetic_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("synthetic_scale");
    for &doc_count in bench_doc_counts() {
        let docs = build_mixed_size_corpus(doc_count);
        let configured = bench_sample_size(10);
        let capped = if doc_count >= 5_000 {
            configured.min(20)
        } else {
            configured
        };
        group.sample_size(capped);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{doc_count}_docs")),
            &docs,
            |b, docs| {
                b.iter(|| {
                    let realm = index_documents_from_slices(docs);
                    black_box(realm.document_count())
                });
            },
        );
    }
}

fn bench_reparse_single_doc(c: &mut Criterion) {
    let content = sample_doc(0);
    c.bench_function("reparse_single_document", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content);
            black_box(index.headings().len())
        })
    });
}

/// Parse + index real large markdown corpus (~492KB). Skips if fixture missing.
fn bench_reparse_real_large_doc(c: &mut Criterion) {
    let path = match real_corpus_path() {
        Some(p) => p,
        None => {
            eprintln!("  [memory] Skipping reparse_real_large_doc: epstein_20250227_all_in_one.md not found");
            return;
        }
    };
    let content = std::fs::read_to_string(&path).expect("read fixture");
    let size_kb = content.len() / 1024;
    eprintln!("  [memory] reparse_real_large_doc: fixture {} KB", size_kb);

    let mut group = c.benchmark_group("real_corpus");
    group.sample_size(bench_sample_size(20));
    group.bench_function("reparse_real_large_doc", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content);
            black_box(index.headings().len())
        })
    });
}

/// Index real corpus split into sections (by ## headings). Each section = 1 doc.
fn bench_index_real_corpus(c: &mut Criterion) {
    let path = match real_corpus_path() {
        Some(p) => p,
        None => {
            eprintln!(
                "  [memory] Skipping index_real_corpus: epstein_20250227_all_in_one.md not found"
            );
            return;
        }
    };
    let content = std::fs::read_to_string(&path).expect("read fixture");
    // Split on "## " to get sections; first segment may be preamble (e.g. "# Data...")
    let sections: Vec<String> = content
        .split("\n## ")
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            if s.starts_with('#') {
                s.to_string()
            } else {
                format!("## {}", s)
            }
        })
        .collect();
    let n = sections.len();
    eprintln!("  [memory] index_real_corpus: {} sections from fixture", n);

    let mut group = c.benchmark_group("real_corpus");
    group.sample_size(bench_sample_size(20));
    group.bench_function("index_real_corpus", |b| {
        b.iter(|| {
            let realm = index_documents_from_slices(&sections);
            black_box(realm.document_count())
        })
    });
}

fn index_documents_from_slices(sections: &[String]) -> RealmIndex {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut parser = Parser::new().expect("parser init");
    let mut realm = RealmIndex::new();

    for (i, content) in sections.iter().enumerate() {
        let uri = DocumentUri::from_file_path(&PathBuf::from(format!("/real/doc{}.md", i)));
        let ast = parser.parse(content).expect("parse");
        let index = DocumentIndex::from_ast(ast);
        rt.block_on(realm.add_document(uri, index));
    }

    realm
}

/// Index documents from file paths. Content loaded once at init.
fn index_documents_from_paths(paths: &[PathBuf], contents: &[String]) -> RealmIndex {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut parser = Parser::new().expect("parser init");
    let mut realm = RealmIndex::new();

    for (path, content) in paths.iter().zip(contents.iter()) {
        let uri = DocumentUri::from_file_path(path);
        let ast = parser.parse(content).expect("parse");
        let index = DocumentIndex::from_ast(ast);
        rt.block_on(realm.add_document(uri, index));
    }

    realm
}

/// Index .md files from a directory (e.g. gigapowers). Skips if dir missing.
/// Use MARKYMARK_BENCH_CORPUS_DIR or defaults to /Volumes/code/gigapowers.
fn bench_index_docs_dir(c: &mut Criterion) {
    let dir = match docs_corpus_dir() {
        Some(d) => d,
        None => {
            eprintln!(
                "  [memory] Skipping index_docs_dir: set MARKYMARK_BENCH_CORPUS_DIR or add /Volumes/code/gigapowers"
            );
            return;
        }
    };

    let paths = collect_md_files(&dir, 1000);
    if paths.is_empty() {
        eprintln!(
            "  [memory] Skipping index_docs_dir: no .md files found in {:?}",
            dir
        );
        return;
    }

    let pairs: Vec<(PathBuf, String)> = paths
        .into_iter()
        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|c| (p, c)))
        .collect();
    let n = pairs.len();
    if n == 0 {
        eprintln!("  [memory] Skipping index_docs_dir: no readable .md files");
        return;
    }
    let (paths, contents): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
    let total_kb = contents.iter().map(|s| s.len()).sum::<usize>() / 1024;
    eprintln!(
        "  [memory] index_docs_dir: {} files, {} KB total from {:?}",
        n, total_kb, dir
    );

    let mut group = c.benchmark_group("real_corpus");
    group.sample_size(bench_sample_size(10));
    group.bench_function("index_docs_dir", |b| {
        b.iter(|| {
            let realm = index_documents_from_paths(&paths, &contents);
            black_box(realm.document_count())
        })
    });
}

/// Peak RSS and resident memory after indexing 100 documents.
fn bench_memory_footprint(c: &mut Criterion) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut group = c.benchmark_group("memory");
    group.sample_size(bench_sample_size(10));
    group.bench_function("memory_after_index_100", |b| {
        b.iter(|| {
            let realm = index_n_documents(100);
            let resident_mb = memory_stats::memory_stats()
                .map(|m| m.physical_mem / (1024 * 1024))
                .unwrap_or(0);
            let peak_rss_kb = get_maxrss_kb();
            if !REPORTED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "  [memory] memory_after_index_100: {} MiB resident, {} KB peak RSS",
                    resident_mb, peak_rss_kb
                );
            }
            black_box(realm);
            black_box((resident_mb, peak_rss_kb))
        })
    });
}

/// Peak RSS (max resident set size) in KB. Unix only.
fn get_maxrss_kb() -> u64 {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::uninit();
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            let usage = unsafe { usage.assume_init() };
            // macOS: bytes; Linux: KB
            #[cfg(target_os = "macos")]
            return (usage.ru_maxrss as u64) / 1024;
            #[cfg(not(target_os = "macos"))]
            return usage.ru_maxrss as u64;
        }
    }
    #[cfg(not(unix))]
    let _ = ();
    0
}

/// Allocation count for indexing 100 documents.
/// Heap allocations (bumpalo uses its own allocation). Run with --nocapture to see count.
fn bench_allocation_count_index_100(c: &mut Criterion) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut group = c.benchmark_group("memory");
    group.sample_size(bench_sample_size(10));
    group.bench_function("alloc_count_index_100", |b| {
        b.iter(|| {
            ALLOC_COUNT.store(0, Ordering::Relaxed);
            let realm = index_n_documents(100);
            let count = ALLOC_COUNT.load(Ordering::Relaxed);
            if !REPORTED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "  [memory] alloc_count_index_100: {} heap allocations",
                    count
                );
            }
            black_box(realm);
            black_box(count)
        })
    });
}

/// Concurrent indexing: N threads each building a realm with 100 docs.
fn bench_concurrent_index_4_threads(c: &mut Criterion) {
    c.bench_function("concurrent_index_4x100_docs", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    thread::spawn(|| {
                        let realm = index_n_documents(100);
                        black_box(realm.document_count())
                    })
                })
                .collect();
            let counts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            black_box(counts)
        })
    });
}

fn bench_concurrent_index_8_threads(c: &mut Criterion) {
    c.bench_function("concurrent_index_8x100_docs", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    thread::spawn(|| {
                        let realm = index_n_documents(100);
                        black_box(realm.document_count())
                    })
                })
                .collect();
            let counts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            black_box(counts)
        })
    });
}

/// Investigation: measure arena creation+destruction overhead in isolation.
/// Compares: (a) Bump::new() + drop, (b) Bump::reset() reuse, (c) full reparse.
/// This quantifies how much of the reparse cost is arena lifecycle vs parsing/indexing.
fn bench_arena_reuse_investigation(c: &mut Criterion) {
    use bumpalo::Bump;

    let content = sample_doc(0);
    let content_large = (0..50)
        .map(|i| sample_doc(i))
        .collect::<Vec<_>>()
        .join("\n");

    let mut group = c.benchmark_group("arena_reuse_investigation");

    // (a) Cost of Bump::new() + drop — arena lifecycle overhead
    group.bench_function("arena_create_drop_small", |b| {
        b.iter(|| {
            let arena = Bump::new();
            // Simulate typical small doc allocation pattern
            for i in 0..20 {
                let _ = arena.alloc_str(&format!("heading-{}", i));
            }
            black_box(&arena);
            drop(arena);
        })
    });

    // (b) Cost of Bump::reset() — reuse pattern
    group.bench_function("arena_reset_reuse_small", |b| {
        let mut arena = Bump::new();
        b.iter(|| {
            arena.reset();
            for i in 0..20 {
                let _ = arena.alloc_str(&format!("heading-{}", i));
            }
            black_box(&arena);
        })
    });

    // (c) Full reparse — to show the ratio
    group.bench_function("full_reparse_small", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content);
            black_box(index.headings().len())
        })
    });

    // (d) Arena lifecycle for larger allocation pattern
    group.bench_function("arena_create_drop_large", |b| {
        b.iter(|| {
            let arena = Bump::new();
            for i in 0..500 {
                let _ = arena.alloc_str(&format!("heading-slug-{}-with-longer-text", i));
            }
            black_box(&arena);
            drop(arena);
        })
    });

    // (e) Arena reuse for larger allocation pattern
    group.bench_function("arena_reset_reuse_large", |b| {
        let mut arena = Bump::new();
        b.iter(|| {
            arena.reset();
            for i in 0..500 {
                let _ = arena.alloc_str(&format!("heading-slug-{}-with-longer-text", i));
            }
            black_box(&arena);
        })
    });

    // (f) Full reparse of large doc
    group.bench_function("full_reparse_large", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content_large);
            black_box(index.headings().len())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_index_10_docs,
    bench_index_100_docs,
    bench_index_synthetic_scale,
    bench_reparse_single_doc,
    bench_reparse_real_large_doc,
    bench_index_real_corpus,
    bench_index_docs_dir,
    bench_memory_footprint,
    bench_allocation_count_index_100,
    bench_concurrent_index_4_threads,
    bench_concurrent_index_8_threads,
    bench_arena_reuse_investigation,
);
criterion_main!(benches);
