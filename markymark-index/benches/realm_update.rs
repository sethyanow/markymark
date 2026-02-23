//! Benchmark: update_document (incremental) vs remove+add (baseline).
//!
//! Measures RealmIndex update performance for the common case:
//! single-char edit in a 50-heading document with no structural change.
//! Uses iter_batched to separate parsing cost from realm index operations.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;
use std::path::PathBuf;

/// Generate a markdown document with `n_headings` headings and `n_tags` tags.
fn generate_doc(n_headings: usize, n_tags: usize) -> String {
    let mut doc = String::with_capacity(n_headings * 60 + n_tags * 20);
    for i in 0..n_headings {
        let level = (i % 3) + 1;
        let hashes = "#".repeat(level);
        doc.push_str(&format!(
            "{hashes} Heading {i}\n\nParagraph content for section {i}.\n\n"
        ));
    }
    for i in 0..n_tags {
        doc.push_str(&format!("#tag{i} "));
    }
    doc.push('\n');
    doc
}

fn parse_doc(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse should succeed");
    DocumentIndex::from_ast(ast)
}

fn bench_update_vs_remove_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("realm_update");

    let doc_text = generate_doc(50, 5);
    // "Edited" text: identical structure, one char changed in body
    let doc_text_edited = doc_text.replace("section 0", "section X");

    let uri = DocumentUri::from_file_path(&PathBuf::from("/vault/bench_doc.md"));

    // Baseline: remove_document + add_document (measures only realm ops, not parsing)
    group.bench_function("remove_add", |b| {
        b.iter_batched(
            || {
                // Setup: fresh realm + fresh parsed index for the "edit"
                let mut realm = RealmIndex::new();
                let index_orig = parse_doc(&doc_text);
                realm.add_document(uri.clone(), index_orig);
                let index_edited = parse_doc(&doc_text_edited);
                (realm, index_edited)
            },
            |(mut realm, index_edited)| {
                // Measured: only the realm remove+add operations
                realm.remove_document(black_box(&uri));
                realm.add_document(black_box(uri.clone()), black_box(index_edited));
            },
            BatchSize::SmallInput,
        );
    });

    // Incremental: update_document (fast path — identical contribution sets)
    group.bench_function("update_no_structural_change", |b| {
        b.iter_batched(
            || {
                // Setup: fresh realm + fresh parsed index for the "edit"
                let mut realm = RealmIndex::new();
                let index_orig = parse_doc(&doc_text);
                realm.add_document(uri.clone(), index_orig);
                let index_edited = parse_doc(&doc_text_edited);
                (realm, index_edited)
            },
            |(mut realm, index_edited)| {
                // Measured: only the update_document operation
                realm.update_document(black_box(uri.clone()), black_box(index_edited));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_update_vs_remove_add);
criterion_main!(benches);
