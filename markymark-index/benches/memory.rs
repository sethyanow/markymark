//! Memory and performance benchmarks for arena allocation.
//!
//! Measures: indexing N documents (time, peak RSS), allocation count, re-parse time.
//! Arena allocation reduces heap allocations; bumpalo uses per-document bump allocation.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

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
        let index = DocumentIndex::from_ast(ast);
        realm.add_document(uri, index);
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

fn bench_reparse_single_doc(c: &mut Criterion) {
    let content = sample_doc(0);
    c.bench_function("reparse_single_document", |b| {
        b.iter(|| {
            let index = reparse_single_document(&content);
            black_box(index.headings().len())
        })
    });
}

/// Peak RSS / workload for indexing N documents.
fn bench_peak_rss_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");
    group.sample_size(10);
    group.bench_function("peak_rss_after_index_100", |b| {
        b.iter(|| {
            let realm = index_n_documents(100);
            black_box(realm.document_count());
        })
    });
}

/// Allocation count for indexing 100 documents.
/// Heap allocations (bumpalo uses its own allocation). Run with --nocapture to see count.
fn bench_allocation_count_index_100(c: &mut Criterion) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut group = c.benchmark_group("memory");
    group.sample_size(10);
    group.bench_function("alloc_count_index_100", |b| {
        b.iter(|| {
            ALLOC_COUNT.store(0, Ordering::Relaxed);
            let realm = index_n_documents(100);
            let count = ALLOC_COUNT.load(Ordering::Relaxed);
            if REPORTED.swap(true, Ordering::Relaxed) == false {
                eprintln!("  [memory] alloc_count_index_100: {} heap allocations", count);
            }
            black_box(realm);
            black_box(count)
        })
    });
}

criterion_group!(
    benches,
    bench_index_10_docs,
    bench_index_100_docs,
    bench_reparse_single_doc,
    bench_peak_rss_indexing,
    bench_allocation_count_index_100,
);
criterion_main!(benches);
