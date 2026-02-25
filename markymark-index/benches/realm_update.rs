//! Benchmark: update_document (incremental) vs remove+add (baseline).
//!
//! Two benchmark groups:
//! 1. `realm_update` — single document in 1-doc realm (regression baseline)
//! 2. `realm_update_vault` — 50KB document in 1000-doc vault (epic criterion spec)
//!
//! Uses iter_batched to separate parsing cost from realm index operations.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;
use std::path::PathBuf;

/// Generate a markdown document with `n_headings` headings and `n_tags` tags.
/// Produces a small document (~3KB) for the original single-doc benchmark.
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

/// Generate a ~50KB markdown document with realistic structure.
/// ~40 headings, ~15 tags, ~5 block IDs, multi-paragraph sections.
fn generate_large_doc(doc_id: usize) -> String {
    let mut doc = String::with_capacity(55_000);

    for i in 0..40 {
        let level = (i % 3) + 1;
        let hashes = "#".repeat(level);
        doc.push_str(&format!("{hashes} Section {doc_id} heading {i}\n\n"));

        // 3 paragraphs per section to reach ~50KB
        for p in 0..3 {
            doc.push_str(&format!(
                "This is paragraph {p} of section {i} in document {doc_id}. \
                 It contains enough text to make the document realistically sized \
                 for benchmarking purposes. The content varies per section to ensure \
                 unique text throughout the document. Lorem ipsum dolor sit amet, \
                 consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut \
                 labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
                 exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\n"
            ));
        }

        // Add block IDs to some sections
        if i % 8 == 0 {
            doc.push_str(&format!("^block-ref-{doc_id}-{i}\n\n"));
        }

        // Add inline code spans
        if i % 4 == 0 {
            doc.push_str(&format!("See `function_{doc_id}_{i}()` for details.\n\n"));
        }
    }

    // Tags at the end
    for i in 0..15 {
        // Mix of shared and unique tags
        if i < 5 {
            doc.push_str(&format!("#shared-tag-{i} "));
        } else {
            doc.push_str(&format!("#doc-{doc_id}-tag-{i} "));
        }
    }
    doc.push('\n');
    doc
}

/// Generate a small vault document (~1KB) for populating the realm.
/// Each doc has ~10 headings, ~3 tags, ~1 block ID.
fn generate_vault_doc(doc_id: usize) -> String {
    let mut doc = String::with_capacity(1200);
    for i in 0..10 {
        let level = (i % 3) + 1;
        let hashes = "#".repeat(level);
        doc.push_str(&format!(
            "{hashes} Doc {doc_id} section {i}\n\nContent for doc {doc_id} section {i}.\n\n"
        ));
    }
    // Block ID
    doc.push_str(&format!("^block-{doc_id}\n\n"));
    // Tags: some shared (~30% overlap), some unique
    doc.push_str(&format!(
        "#project #status-{} #topic-{}\n",
        doc_id % 3,  // 3 shared status tags
        doc_id % 20  // 20 shared topic tags
    ));
    doc
}

fn parse_doc(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse should succeed");
    DocumentIndex::from_ast(ast)
}

/// Pre-populate a realm with `n_docs` vault documents.
/// Returns the realm and the parser (for reuse).
fn build_vault(n_docs: usize) -> RealmIndex {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut realm = RealmIndex::new();
    let mut parser = Parser::new().expect("parser init");

    for i in 0..n_docs {
        let uri = DocumentUri::from_file_path(&PathBuf::from(format!("/vault/doc_{i}.md")));
        let text = generate_vault_doc(i);
        let ast = parser.parse(&text).expect("parse should succeed");
        let index = DocumentIndex::from_ast(ast);
        rt.block_on(realm.add_document(uri, index));
    }
    realm
}

// ── Original single-doc benchmarks (regression baseline) ──

fn bench_update_vs_remove_add(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
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
                rt.block_on(realm.add_document(uri.clone(), index_orig));
                let index_edited = parse_doc(&doc_text_edited);
                (realm, index_edited)
            },
            |(mut realm, index_edited)| {
                // Measured: only the realm remove+add operations
                realm.remove_document(black_box(&uri));
                rt.block_on(realm.add_document(black_box(uri.clone()), black_box(index_edited)));
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
                rt.block_on(realm.add_document(uri.clone(), index_orig));
                let index_edited = parse_doc(&doc_text_edited);
                (realm, index_edited)
            },
            |(mut realm, index_edited)| {
                // Measured: only the update_document operation
                rt.block_on(realm.update_document(black_box(uri.clone()), black_box(index_edited)));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ── Vault benchmarks (epic criterion spec: 50KB doc, 1000-doc vault) ──

fn bench_vault_update(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("realm_update_vault");
    // Reduce sample count — vault construction is expensive
    group.sample_size(20);

    let target_uri = DocumentUri::from_file_path(&PathBuf::from("/vault/target_doc.md"));

    // Generate 50KB target document and its "edited" version (text only — parsed fresh per iteration)
    let large_doc = generate_large_doc(9999);
    assert!(
        large_doc.len() >= 40_000 && large_doc.len() <= 60_000,
        "large_doc size out of range: {} bytes",
        large_doc.len()
    );
    // Edit: change one word in body text (no structural change)
    let large_doc_edited = large_doc.replace("document 9999", "document XXXX");
    // Edit with tag change: replace a tag (structural change in tags)
    let large_doc_tag_changed = large_doc.replace("#shared-tag-0", "#new-unique-tag");

    // IMPORTANT: Return realm from routine closures so criterion drops it
    // OUTSIDE the timing window. Otherwise realm drop (~2ms for 1001 arenas)
    // dominates and masks the actual operation cost difference.

    // Case 1: Baseline — remove + add in 1000-doc vault
    {
        let doc_orig = large_doc.clone();
        let doc_edit = large_doc_edited.clone();
        let uri = target_uri.clone();
        group.bench_function("vault_1000_remove_add", |b| {
            b.iter_batched(
                || {
                    let mut realm = build_vault(1000);
                    let orig_index = parse_doc(&doc_orig);
                    rt.block_on(realm.add_document(uri.clone(), orig_index));
                    let edited_index = parse_doc(&doc_edit);
                    (realm, edited_index)
                },
                |(mut realm, new_index)| {
                    realm.remove_document(black_box(&uri));
                    rt.block_on(realm.add_document(black_box(uri.clone()), black_box(new_index)));
                    realm // return so drop is outside timing
                },
                BatchSize::LargeInput,
            );
        });
    }

    // Case 2: Incremental fast path — no structural change in 1000-doc vault
    {
        let doc_orig = large_doc.clone();
        let doc_edit = large_doc_edited.clone();
        let uri = target_uri.clone();
        group.bench_function("vault_1000_update_no_change", |b| {
            b.iter_batched(
                || {
                    let mut realm = build_vault(1000);
                    let orig_index = parse_doc(&doc_orig);
                    rt.block_on(realm.add_document(uri.clone(), orig_index));
                    let edited_index = parse_doc(&doc_edit);
                    (realm, edited_index)
                },
                |(mut realm, new_index)| {
                    rt.block_on(realm.update_document(black_box(uri.clone()), black_box(new_index)));
                    realm // return so drop is outside timing
                },
                BatchSize::LargeInput,
            );
        });
    }

    // Case 3: Incremental slow path — tag change in 1000-doc vault
    {
        let doc_orig = large_doc;
        let doc_edit = large_doc_tag_changed;
        let uri = target_uri;
        group.bench_function("vault_1000_update_tag_change", |b| {
            b.iter_batched(
                || {
                    let mut realm = build_vault(1000);
                    let orig_index = parse_doc(&doc_orig);
                    rt.block_on(realm.add_document(uri.clone(), orig_index));
                    let edited_index = parse_doc(&doc_edit);
                    (realm, edited_index)
                },
                |(mut realm, new_index)| {
                    rt.block_on(realm.update_document(black_box(uri.clone()), black_box(new_index)));
                    realm // return so drop is outside timing
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_update_vs_remove_add, bench_vault_update);
criterion_main!(benches);
