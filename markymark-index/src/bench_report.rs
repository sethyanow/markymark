//! Benchmark ROI report helpers.

/// One scale-row in the ROI table.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleRow {
    /// Document count.
    pub doc_count: usize,
    /// Mean runtime in milliseconds.
    pub mean_ms: f64,
}

/// Summary for one tier run.
#[derive(Debug, Clone, PartialEq)]
pub struct TierSummary {
    /// Tier label (light, medium, ...).
    pub tier: String,
    /// Scale rows for this tier.
    pub rows: Vec<ScaleRow>,
}

/// Snapshot of memory-related benchmark metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemorySnapshot {
    /// Heap allocation count from alloc_count_index_100.
    pub alloc_count: u64,
    /// Resident memory in MiB from memory_after_index_100.
    pub resident_mib: u64,
    /// Peak RSS in KB from memory_after_index_100.
    pub peak_rss_kb: u64,
}

/// Parse Criterion `estimates.json` mean point estimate.
///
/// Returns milliseconds (Criterion stores time in nanoseconds).
#[must_use]
pub fn parse_mean_ms_from_estimates(json: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let ns = value.get("mean")?.get("point_estimate")?.as_f64()?;
    Some(ns / 1_000_000.0)
}

/// Render tiered synthetic-scale benchmark summaries as markdown.
#[must_use]
pub fn render_tier_report(summaries: &[TierSummary], sample_tier: &str) -> String {
    let mut out = String::new();
    out.push_str("# Benchmark ROI Report\n\n");
    out.push_str(&format!(
        "- Sample tier: `{}`\n- Benchmark: `synthetic_scale`\n\n",
        sample_tier
    ));
    out.push_str("| Tier | Docs | Mean (ms) |\n");
    out.push_str("|------|------:|----------:|\n");

    for summary in summaries {
        for row in &summary.rows {
            out.push_str(&format!(
                "| {} | {} | {:.3} |\n",
                summary.tier, row.doc_count, row.mean_ms
            ));
        }
    }

    out
}

/// Parse memory snapshot metrics from benchmark output.
#[must_use]
pub fn parse_memory_snapshot(output: &str) -> MemorySnapshot {
    let mut snapshot = MemorySnapshot::default();

    for line in output.lines() {
        if let Some(rest) = line.split("alloc_count_index_100:").nth(1) {
            let value = rest.split_whitespace().next().unwrap_or("");
            if let Ok(parsed) = value.parse::<u64>() {
                snapshot.alloc_count = parsed;
            }
        }

        if let Some(rest) = line.split("memory_after_index_100:").nth(1) {
            let mut parts = rest.split(',');
            if let Some(resident_part) = parts.next() {
                let resident = resident_part
                    .split_whitespace()
                    .find_map(|token| token.parse::<u64>().ok());
                if let Some(parsed) = resident {
                    snapshot.resident_mib = parsed;
                }
            }
            if let Some(rss_part) = parts.next() {
                let rss = rss_part
                    .split_whitespace()
                    .find_map(|token| token.parse::<u64>().ok());
                if let Some(parsed) = rss {
                    snapshot.peak_rss_kb = parsed;
                }
            }
        }
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::{
        parse_mean_ms_from_estimates, parse_memory_snapshot, render_tier_report, MemorySnapshot,
        ScaleRow, TierSummary,
    };

    #[test]
    fn parses_mean_ns_to_ms() {
        let json =
            r#"{"mean":{"point_estimate":2921523441.6},"median":{"point_estimate":2916658771.0}}"#;
        let ms = parse_mean_ms_from_estimates(json).expect("mean");
        assert!((ms - 2921.5234416).abs() < 0.000001);
    }

    #[test]
    fn invalid_json_returns_none() {
        assert_eq!(parse_mean_ms_from_estimates("{oops"), None);
    }

    #[test]
    fn missing_fields_return_none() {
        assert_eq!(parse_mean_ms_from_estimates("{}"), None);
    }

    #[test]
    fn renders_markdown_table() {
        let summaries = vec![
            TierSummary {
                tier: "light".to_string(),
                rows: vec![ScaleRow {
                    doc_count: 100,
                    mean_ms: 250.0,
                }],
            },
            TierSummary {
                tier: "medium".to_string(),
                rows: vec![
                    ScaleRow {
                        doc_count: 100,
                        mean_ms: 260.0,
                    },
                    ScaleRow {
                        doc_count: 1000,
                        mean_ms: 2900.0,
                    },
                ],
            },
        ];

        let md = render_tier_report(&summaries, "light");
        assert!(md.contains("# Benchmark ROI Report"));
        assert!(md.contains("| Tier | Docs | Mean (ms) |"));
        assert!(md.contains("| light | 100 | 250.000 |"));
        assert!(md.contains("| medium | 1000 | 2900.000 |"));
    }

    #[test]
    fn parses_memory_snapshot_lines() {
        let output = r#"
  [memory] memory_after_index_100: 638 MiB resident, 653312 KB peak RSS
  [memory] alloc_count_index_100: 215837 heap allocations
"#;
        let snapshot = parse_memory_snapshot(output);
        assert_eq!(
            snapshot,
            MemorySnapshot {
                alloc_count: 215_837,
                resident_mib: 638,
                peak_rss_kb: 653_312
            }
        );
    }
}
