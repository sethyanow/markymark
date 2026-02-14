//! Pre-arena baseline benchmarks (476795e).
//!
//! Captures baseline metrics before bumpalo arena migration. API: from_ast(&ast).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

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
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;
use std::path::PathBuf;

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

fn index_n_documents(n: usize) -> RealmIndex {
    let mut parser = Parser::new().expect("parser init");
    let mut realm = RealmIndex::new();

    for i in 0..n {
        let uri = DocumentUri::from_file_path(&PathBuf::from(format!("/vault/doc{}.md", i)));
        let content = sample_doc(i);
        let ast = parser.parse(&content).expect("parse");
        let index = DocumentIndex::from_ast(&ast);
        realm.add_document(uri, index);
    }
    realm
}

fn reparse_single_document(content: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(content).expect("parse");
    DocumentIndex::from_ast(&ast)
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

fn bench_reparse_single_doc(c: &mut Criterion) {
    let content = sample_doc(0);
    c.bench_function("reparse_single_document", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content);
            black_box(index.headings().len())
        })
    });
}

fn get_maxrss_kb() -> u64 {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::uninit();
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            let usage = unsafe { usage.assume_init() };
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

fn bench_memory_footprint(c: &mut Criterion) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut group = c.benchmark_group("memory");
    group.sample_size(10);
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

fn bench_allocation_count_index_100(c: &mut Criterion) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut group = c.benchmark_group("memory");
    group.sample_size(10);
    group.bench_function("alloc_count_index_100", |b| {
        b.iter(|| {
            ALLOC_COUNT.store(0, Ordering::Relaxed);
            let realm = index_n_documents(100);
            let count = ALLOC_COUNT.load(Ordering::Relaxed);
            if !REPORTED.swap(true, Ordering::Relaxed) {
                eprintln!("  [memory] alloc_count_index_100: {} heap allocations", count);
            }
            black_box(realm);
            black_box(count)
        })
    });
}

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

criterion_group!(
    benches,
    bench_index_10_docs,
    bench_index_100_docs,
    bench_reparse_single_doc,
    bench_memory_footprint,
    bench_allocation_count_index_100,
    bench_concurrent_index_4_threads,
    bench_concurrent_index_8_threads,
);
criterion_main!(benches);
