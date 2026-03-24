//! Benchmark: index construction via EngineExtraction vs direct arena decode.
//!
//! Compares two DocumentIndex construction paths across size tiers:
//! 1. `via_extraction` — `to_extraction()` + `from_engine_result_with_frontmatter()`
//! 2. `direct` — `from_engine_result_direct()` (bypasses EngineExtraction)
//!
//! Uses the project's `bench_corpus` infrastructure for realistic structural density.
//! Engine creation and md4c parsing are in setup (not measured).

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use markymark_index::{
    bench_sample_size, build_sized_doc, parse_frontmatter_owned, DocSizeTier, DocumentIndex,
};
use markymark_kernels::engine::DocumentEngine;

fn bench_index_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_construction");
    group.sample_size(bench_sample_size(100));

    for &(label, tier) in &[
        ("small_1k", DocSizeTier::Small),
        ("medium_10k", DocSizeTier::Medium),
        ("large_100k", DocSizeTier::Large),
    ] {
        let doc = build_sized_doc(0, tier);
        let engine = DocumentEngine::new(&doc).unwrap();
        let (fm_base, aliases_base) = parse_frontmatter_owned(&doc);

        group.bench_with_input(BenchmarkId::new("via_extraction", label), &label, |b, _| {
            b.iter_batched(
                || {
                    let result = engine.get_result().unwrap();
                    (result, fm_base.clone(), aliases_base.clone())
                },
                |(result, fm, aliases)| {
                    let extraction = result.to_extraction().unwrap();
                    black_box(DocumentIndex::from_engine_result_with_frontmatter(
                        &extraction,
                        fm,
                        aliases,
                    ));
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("direct", label), &label, |b, _| {
            b.iter_batched(
                || {
                    let result = engine.get_result().unwrap();
                    (result, fm_base.clone(), aliases_base.clone())
                },
                |(result, fm, aliases)| {
                    black_box(
                        DocumentIndex::from_engine_result_direct(&result, fm, aliases).unwrap(),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_index_construction);
criterion_main!(benches);
