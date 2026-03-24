//! Benchmark: index construction via EngineExtraction vs direct arena decode.
//!
//! Compares two DocumentIndex construction paths on a ~50KB document:
//! 1. `via_extraction` — `to_extraction()` + `from_engine_result_with_frontmatter()`
//! 2. `direct` — `from_engine_result_direct()` (bypasses EngineExtraction)
//!
//! Engine creation and md4c parsing are in setup (not measured).
//! Only index construction is benchmarked.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use markymark_index::{parse_frontmatter_owned, DocumentIndex};
use markymark_kernels::engine::DocumentEngine;

/// Generate a ~50KB markdown document with realistic structure.
/// ~40 headings, ~15 tags, ~5 block IDs, multi-paragraph sections.
/// Duplicated from realm_update.rs — benchmark files are standalone.
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
        if i < 5 {
            doc.push_str(&format!("#shared-tag-{i} "));
        } else {
            doc.push_str(&format!("#doc-{doc_id}-tag-{i} "));
        }
    }
    doc.push('\n');
    doc
}

fn bench_index_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_construction");

    // Shared setup: engine creation + frontmatter parsing (done once, not measured)
    let doc = generate_large_doc(0);
    let engine = DocumentEngine::new(&doc).unwrap();
    let (fm_base, aliases_base) = parse_frontmatter_owned(&doc);

    // Old path: to_extraction() + from_engine_result_with_frontmatter()
    group.bench_function("via_extraction", |b| {
        b.iter_batched(
            || {
                // Setup per iteration: fresh EngineResult + cloned frontmatter
                let result = engine.get_result().unwrap();
                (result, fm_base.clone(), aliases_base.clone())
            },
            |(result, fm, aliases)| {
                // Measured: extraction creation + index construction
                let extraction = result.to_extraction().unwrap();
                black_box(DocumentIndex::from_engine_result_with_frontmatter(
                    &extraction, fm, aliases,
                ));
            },
            BatchSize::SmallInput,
        );
    });

    // New path: from_engine_result_direct()
    group.bench_function("direct", |b| {
        b.iter_batched(
            || {
                // Setup per iteration: fresh EngineResult + cloned frontmatter
                let result = engine.get_result().unwrap();
                (result, fm_base.clone(), aliases_base.clone())
            },
            |(result, fm, aliases)| {
                // Measured: direct arena decode (no EngineExtraction intermediary)
                black_box(
                    DocumentIndex::from_engine_result_direct(&result, fm, aliases).unwrap(),
                );
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_index_construction);
criterion_main!(benches);
