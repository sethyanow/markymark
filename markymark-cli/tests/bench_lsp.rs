//! Performance benchmark: markymark vs marksman LSP latency and throughput.
//!
//! Spawns each server, sends identical LSP requests, and measures timing.
//! Outputs JSON + markdown summary. Skips gracefully if marksman is unavailable.
//!
//! Run: `cargo test -p markymark-cli --test bench_lsp -- --nocapture`

mod alignment_support;

use alignment_support::{corpus_dir, markymark_bin, path_to_uri, run_with_timeout, LspProcess};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Benchmark configuration
// ---------------------------------------------------------------------------

const WARMUP_ITERATIONS: usize = 2;
const MEASURED_ITERATIONS: usize = 10;

// ---------------------------------------------------------------------------
// BenchResult — timing for a single method
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BenchResult {
    method: String,
    file: String,
    durations_us: Vec<u64>,
}

impl BenchResult {
    fn p50(&self) -> u64 {
        percentile(&self.durations_us, 50)
    }
    fn p95(&self) -> u64 {
        percentile(&self.durations_us, 95)
    }
    fn p99(&self) -> u64 {
        percentile(&self.durations_us, 99)
    }
    fn mean(&self) -> u64 {
        if self.durations_us.is_empty() {
            return 0;
        }
        self.durations_us.iter().sum::<u64>() / self.durations_us.len() as u64
    }
    fn req_per_sec(&self) -> f64 {
        if self.durations_us.is_empty() {
            return 0.0;
        }
        let mean_secs = self.mean() as f64 / 1_000_000.0;
        if mean_secs == 0.0 {
            return 0.0;
        }
        1.0 / mean_secs
    }
}

fn percentile(sorted: &[u64], pct: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let mut v = sorted.to_vec();
    v.sort();
    let idx = (pct as f64 / 100.0 * (v.len() - 1) as f64).round() as usize;
    v[idx.min(v.len() - 1)]
}

// ---------------------------------------------------------------------------
// BenchReport — full benchmark output
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BenchReport {
    markymark_results: Vec<BenchResult>,
    marksman_results: Vec<BenchResult>,
    warmup_iterations: usize,
    measured_iterations: usize,
}

impl BenchReport {
    fn to_json(&self) -> Value {
        let mk_entry = |r: &BenchResult| -> Value {
            json!({
                "method": r.method,
                "file": r.file,
                "samples": r.durations_us.len(),
                "latency_us": {
                    "p50": r.p50(),
                    "p95": r.p95(),
                    "p99": r.p99(),
                    "mean": r.mean(),
                },
                "throughput_rps": format!("{:.1}", r.req_per_sec()),
            })
        };

        json!({
            "benchmark": {
                "warmup_iterations": self.warmup_iterations,
                "measured_iterations": self.measured_iterations,
            },
            "markymark": self.markymark_results.iter().map(mk_entry).collect::<Vec<_>>(),
            "marksman": self.marksman_results.iter().map(mk_entry).collect::<Vec<_>>(),
            "comparison": self.comparison_table(),
        })
    }

    fn comparison_table(&self) -> Vec<Value> {
        let ms_map: BTreeMap<String, &BenchResult> = self
            .marksman_results
            .iter()
            .map(|r| (r.method.clone(), r))
            .collect();

        self.markymark_results
            .iter()
            .map(|mm| {
                let ms = ms_map.get(&mm.method);
                let speedup = ms.map(|ms| {
                    if mm.mean() == 0 {
                        0.0
                    } else {
                        ms.mean() as f64 / mm.mean() as f64
                    }
                });
                json!({
                    "method": mm.method,
                    "markymark_p50_us": mm.p50(),
                    "marksman_p50_us": ms.map(|m| m.p50()).unwrap_or(0),
                    "markymark_p95_us": mm.p95(),
                    "marksman_p95_us": ms.map(|m| m.p95()).unwrap_or(0),
                    "speedup_vs_marksman": speedup.map(|s| format!("{:.2}x", s)).unwrap_or_else(|| "N/A".to_string()),
                })
            })
            .collect()
    }

    fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# LSP Performance Benchmark\n\n");
        out.push_str(&format!(
            "Warmup: {} iterations, Measured: {} iterations\n\n",
            self.warmup_iterations, self.measured_iterations
        ));

        out.push_str("## Results\n\n");
        out.push_str(
            "| Method | markymark p50 (µs) | marksman p50 (µs) | markymark p95 (µs) | marksman p95 (µs) | Speedup |\n",
        );
        out.push_str(
            "|--------|-------------------:|------------------:|-------------------:|------------------:|--------:|\n",
        );

        let ms_map: BTreeMap<String, &BenchResult> = self
            .marksman_results
            .iter()
            .map(|r| (r.method.clone(), r))
            .collect();

        for mm in &self.markymark_results {
            let ms = ms_map.get(&mm.method);
            let speedup = ms.map(|ms| {
                if mm.mean() == 0 {
                    "N/A".to_string()
                } else {
                    format!("{:.2}x", ms.mean() as f64 / mm.mean() as f64)
                }
            });
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                mm.method,
                mm.p50(),
                ms.map(|m| m.p50()).unwrap_or(0),
                mm.p95(),
                ms.map(|m| m.p95()).unwrap_or(0),
                speedup.unwrap_or_else(|| "N/A".to_string()),
            ));
        }

        out.push_str("\n## Throughput\n\n");
        out.push_str("| Method | markymark (req/s) | marksman (req/s) |\n");
        out.push_str("|--------|------------------:|-----------------:|\n");

        for mm in &self.markymark_results {
            let ms = ms_map.get(&mm.method);
            out.push_str(&format!(
                "| {} | {:.1} | {:.1} |\n",
                mm.method,
                mm.req_per_sec(),
                ms.map(|m| m.req_per_sec()).unwrap_or(0.0),
            ));
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------------------------

struct BenchScenario {
    method: &'static str,
    file: &'static str,
    params_fn: Box<dyn Fn(&str) -> Value>,
}

fn benchmark_scenarios() -> Vec<BenchScenario> {
    vec![
        BenchScenario {
            method: "textDocument/definition",
            file: "links.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 4, "character": 25 }
                })
            }),
        },
        BenchScenario {
            method: "textDocument/references",
            file: "basic.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 2 },
                    "context": { "includeDeclaration": true }
                })
            }),
        },
        BenchScenario {
            method: "textDocument/hover",
            file: "basic.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 4, "character": 5 }
                })
            }),
        },
        BenchScenario {
            method: "textDocument/completion",
            file: "links.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 4, "character": 24 }
                })
            }),
        },
        BenchScenario {
            method: "textDocument/rename",
            file: "basic.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 4, "character": 5 },
                    "newName": "Renamed Section"
                })
            }),
        },
        BenchScenario {
            method: "textDocument/documentSymbol",
            file: "basic.md",
            params_fn: Box::new(|uri| {
                json!({
                    "textDocument": { "uri": uri }
                })
            }),
        },
        BenchScenario {
            method: "workspace/symbol",
            file: "(workspace)",
            params_fn: Box::new(|_| json!({ "query": "Section" })),
        },
    ]
}

fn run_benchmark(proc: &mut LspProcess, scenario: &BenchScenario, corpus: &Path) -> BenchResult {
    let uri = if scenario.file == "(workspace)" {
        String::new()
    } else {
        path_to_uri(&corpus.join(scenario.file))
    };

    // Warmup
    for _ in 0..WARMUP_ITERATIONS {
        let params = (scenario.params_fn)(&uri);
        let _ = proc.send_request(scenario.method, params);
    }

    // Measured iterations
    let mut durations_us = Vec::with_capacity(MEASURED_ITERATIONS);
    for _ in 0..MEASURED_ITERATIONS {
        let params = (scenario.params_fn)(&uri);
        let start = Instant::now();
        let _ = proc.send_request(scenario.method, params);
        let elapsed = start.elapsed();
        durations_us.push(elapsed.as_micros() as u64);
    }

    BenchResult {
        method: scenario.method.to_string(),
        file: scenario.file.to_string(),
        durations_us,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_benchmark_markymark_only() {
    run_with_timeout(|| {
        let corpus = corpus_dir();
        let mm_bin = markymark_bin();

        let mut mm = LspProcess::spawn(&mm_bin, &["--lsp"], &corpus, "markymark");
        let corpus_files = [
            "basic.md",
            "links.md",
            "cross-refs.md",
            "edge-cases.md",
            "xml-tags.md",
        ];
        for f in &corpus_files {
            let p = corpus.join(f);
            if p.exists() {
                mm.open_file(&p);
            }
        }
        mm.drain_notifications();

        let scenarios = benchmark_scenarios();
        let mut results = Vec::new();
        for s in &scenarios {
            results.push(run_benchmark(&mut mm, s, &corpus));
        }

        // Validate output
        assert_eq!(results.len(), 7, "should have 7 benchmark results");
        for r in &results {
            assert!(
                !r.durations_us.is_empty(),
                "{}: should have timing samples",
                r.method
            );
            assert!(r.p50() > 0, "{}: p50 should be > 0", r.method);
        }

        let report = BenchReport {
            markymark_results: results,
            marksman_results: Vec::new(),
            warmup_iterations: WARMUP_ITERATIONS,
            measured_iterations: MEASURED_ITERATIONS,
        };

        let json = report.to_json();
        assert!(json.get("markymark").is_some());
        assert!(json.get("benchmark").is_some());

        let md = report.to_markdown();
        assert!(md.contains("LSP Performance Benchmark"));
        assert!(md.contains("textDocument/definition"));

        eprintln!("\n{md}");
        eprintln!("JSON:\n{}", serde_json::to_string_pretty(&json).unwrap());

        mm.shutdown_and_exit();
    });
}

#[test]
fn test_benchmark_dual_process() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();

        let corpus = corpus_dir();
        let mm_bin = markymark_bin();

        let mut mm = LspProcess::spawn(&mm_bin, &["--lsp"], &corpus, "markymark");
        let mut ms = LspProcess::spawn(&marksman_path, &["server"], &corpus, "marksman");

        let corpus_files = [
            "basic.md",
            "links.md",
            "cross-refs.md",
            "edge-cases.md",
            "xml-tags.md",
        ];
        for f in &corpus_files {
            let p = corpus.join(f);
            if p.exists() {
                mm.open_file(&p);
                ms.open_file(&p);
            }
        }
        mm.drain_notifications();
        ms.drain_notifications();

        let scenarios = benchmark_scenarios();
        let mut mm_results = Vec::new();
        let mut ms_results = Vec::new();

        for s in &scenarios {
            mm_results.push(run_benchmark(&mut mm, s, &corpus));
            ms_results.push(run_benchmark(&mut ms, s, &corpus));
        }

        let report = BenchReport {
            markymark_results: mm_results,
            marksman_results: ms_results,
            warmup_iterations: WARMUP_ITERATIONS,
            measured_iterations: MEASURED_ITERATIONS,
        };

        // Validate JSON report schema
        let json = report.to_json();
        assert!(
            json.get("markymark").is_some(),
            "JSON should have markymark results"
        );
        assert!(
            json.get("marksman").is_some(),
            "JSON should have marksman results"
        );
        assert!(
            json.get("comparison").is_some(),
            "JSON should have comparison table"
        );
        assert!(
            json.get("benchmark").is_some(),
            "JSON should have benchmark metadata"
        );

        let mm_entries = json["markymark"].as_array().unwrap();
        assert_eq!(mm_entries.len(), 7, "should have 7 markymark benchmarks");
        for entry in mm_entries {
            assert!(entry.get("method").is_some(), "entry should have method");
            assert!(
                entry.get("latency_us").is_some(),
                "entry should have latency_us"
            );
            assert!(
                entry["latency_us"].get("p50").is_some(),
                "latency should have p50"
            );
            assert!(
                entry["latency_us"].get("p95").is_some(),
                "latency should have p95"
            );
            assert!(
                entry["latency_us"].get("p99").is_some(),
                "latency should have p99"
            );
            assert!(
                entry.get("throughput_rps").is_some(),
                "entry should have throughput_rps"
            );
            assert!(entry.get("samples").is_some(), "entry should have samples");
        }

        let md = report.to_markdown();
        assert!(
            md.contains("LSP Performance Benchmark"),
            "markdown should have title"
        );
        assert!(
            md.contains("Speedup"),
            "markdown should have speedup column"
        );
        assert!(
            md.contains("Throughput"),
            "markdown should have throughput section"
        );

        // Write artifacts to working directory for CI upload
        let artifact_dir = std::env::var("BENCH_ARTIFACT_DIR").ok();
        if let Some(dir) = artifact_dir {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).ok();
            std::fs::write(
                dir.join("benchmark-results.json"),
                serde_json::to_string_pretty(&json).unwrap(),
            )
            .ok();
            std::fs::write(dir.join("benchmark-results.md"), &md).ok();
        }

        eprintln!("\n{md}");
        eprintln!("JSON:\n{}", serde_json::to_string_pretty(&json).unwrap());

        mm.shutdown_and_exit();
        ms.shutdown_and_exit();
    });
}

#[test]
fn test_benchmark_report_schema_validation() {
    // Validate that BenchReport always produces well-formed output
    let report = BenchReport {
        markymark_results: vec![BenchResult {
            method: "textDocument/hover".to_string(),
            file: "basic.md".to_string(),
            durations_us: vec![100, 200, 150, 180, 120],
        }],
        marksman_results: vec![BenchResult {
            method: "textDocument/hover".to_string(),
            file: "basic.md".to_string(),
            durations_us: vec![500, 600, 550, 580, 520],
        }],
        warmup_iterations: 2,
        measured_iterations: 5,
    };

    let json = report.to_json();

    // Schema validation: required top-level keys
    assert!(json.get("benchmark").is_some(), "missing 'benchmark' key");
    assert!(json.get("markymark").is_some(), "missing 'markymark' key");
    assert!(json.get("marksman").is_some(), "missing 'marksman' key");
    assert!(json.get("comparison").is_some(), "missing 'comparison' key");

    // Schema validation: benchmark metadata
    let bench = &json["benchmark"];
    assert_eq!(bench["warmup_iterations"], 2);
    assert_eq!(bench["measured_iterations"], 5);

    // Schema validation: each entry has required fields
    for entry in json["markymark"].as_array().unwrap() {
        assert!(entry.get("method").is_some(), "missing method");
        assert!(entry.get("file").is_some(), "missing file");
        assert!(entry.get("samples").is_some(), "missing samples");
        assert!(entry.get("latency_us").is_some(), "missing latency_us");
        assert!(
            entry.get("throughput_rps").is_some(),
            "missing throughput_rps"
        );
        let lat = &entry["latency_us"];
        assert!(lat.get("p50").is_some(), "missing p50");
        assert!(lat.get("p95").is_some(), "missing p95");
        assert!(lat.get("p99").is_some(), "missing p99");
        assert!(lat.get("mean").is_some(), "missing mean");
    }

    // Schema validation: comparison entries
    for comp in json["comparison"].as_array().unwrap() {
        assert!(comp.get("method").is_some(), "missing method in comparison");
        assert!(
            comp.get("markymark_p50_us").is_some(),
            "missing markymark_p50_us"
        );
        assert!(
            comp.get("marksman_p50_us").is_some(),
            "missing marksman_p50_us"
        );
        assert!(comp.get("speedup_vs_marksman").is_some(), "missing speedup");
    }

    // Markdown validation
    let md = report.to_markdown();
    assert!(md.contains("LSP Performance Benchmark"));
    assert!(md.contains("textDocument/hover"));
    assert!(md.contains("Speedup"));
    assert!(md.contains("Throughput"));
    assert!(md.contains("req/s"));
}
