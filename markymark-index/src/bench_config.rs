//! Benchmark sampling configuration helpers.
//!
//! Supports tier names and numeric overrides through environment variables.

/// Resolve Criterion sample size using project env overrides.
///
/// Priority:
/// 1. `MARKYMARK_BENCH_SAMPLES`:
///    - `light` => 10
///    - `medium` => 50
///    - `heavy` => 100
///    - positive integer => that value
/// 2. `MARKYMARK_BENCH_HEAVY` (legacy) => 100
/// 3. provided `default`
#[must_use]
pub fn bench_sample_size(default: usize) -> usize {
    if let Ok(raw) = std::env::var("MARKYMARK_BENCH_SAMPLES") {
        let value = raw.trim().to_ascii_lowercase();
        return match value.as_str() {
            "light" => 10,
            "medium" => 50,
            "heavy" => 100,
            _ => value
                .parse::<usize>()
                .ok()
                .filter(|n| *n >= 10)
                .unwrap_or(default),
        };
    }

    if std::env::var("MARKYMARK_BENCH_HEAVY").is_ok() {
        100
    } else {
        default
    }
}

/// Resolve benchmark corpus-size tiers for scaling runs.
///
/// `MARKYMARK_BENCH_DOC_TIER` values:
/// - `light` => `[100]`
/// - `medium` => `[100, 1_000]`
/// - `heavy` => `[100, 1_000, 5_000]`
/// - `extreme` => `[100, 1_000, 5_000, 10_000]`
///
/// Missing or invalid values default to `medium`.
#[must_use]
pub fn bench_doc_counts() -> &'static [usize] {
    match std::env::var("MARKYMARK_BENCH_DOC_TIER")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("light") => &[100],
        Some("medium") => &[100, 1_000],
        Some("heavy") => &[100, 1_000, 5_000],
        Some("extreme") => &[100, 1_000, 5_000, 10_000],
        Some(_) | None => &[100, 1_000],
    }
}

#[cfg(test)]
mod tests {
    use super::{bench_doc_counts, bench_sample_size};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env(
        samples: Option<&str>,
        heavy: Option<&str>,
        doc_tier: Option<&str>,
        f: impl FnOnce(),
    ) {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let prev_samples = std::env::var("MARKYMARK_BENCH_SAMPLES").ok();
        let prev_heavy = std::env::var("MARKYMARK_BENCH_HEAVY").ok();
        let prev_doc_tier = std::env::var("MARKYMARK_BENCH_DOC_TIER").ok();

        match samples {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_SAMPLES", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_SAMPLES") },
        }
        match heavy {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_HEAVY", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_HEAVY") },
        }
        match doc_tier {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_DOC_TIER", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_DOC_TIER") },
        }

        f();

        match prev_samples {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_SAMPLES", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_SAMPLES") },
        }
        match prev_heavy {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_HEAVY", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_HEAVY") },
        }
        match prev_doc_tier {
            Some(v) => unsafe { std::env::set_var("MARKYMARK_BENCH_DOC_TIER", v) },
            None => unsafe { std::env::remove_var("MARKYMARK_BENCH_DOC_TIER") },
        }
    }

    #[test]
    fn uses_default_when_no_env_is_set() {
        with_env(None, None, None, || {
            assert_eq!(bench_sample_size(20), 20);
        });
    }

    #[test]
    fn supports_legacy_heavy_flag() {
        with_env(None, Some("1"), None, || {
            assert_eq!(bench_sample_size(20), 100);
        });
    }

    #[test]
    fn supports_tier_names() {
        with_env(Some("light"), None, None, || {
            assert_eq!(bench_sample_size(20), 10)
        });
        with_env(Some("medium"), None, None, || {
            assert_eq!(bench_sample_size(20), 50)
        });
        with_env(Some("heavy"), None, None, || {
            assert_eq!(bench_sample_size(20), 100)
        });
    }

    #[test]
    fn supports_numeric_override() {
        with_env(Some("75"), None, None, || {
            assert_eq!(bench_sample_size(20), 75);
        });
    }

    #[test]
    fn invalid_values_fallback_to_default() {
        with_env(Some("banana"), None, None, || {
            assert_eq!(bench_sample_size(20), 20);
        });
    }

    #[test]
    fn explicit_samples_env_takes_precedence_over_legacy_heavy() {
        with_env(Some("50"), Some("1"), None, || {
            assert_eq!(bench_sample_size(20), 50);
        });
    }

    #[test]
    fn doc_tier_defaults_to_medium() {
        with_env(None, None, None, || {
            assert_eq!(bench_doc_counts(), &[100, 1_000]);
        });
    }

    #[test]
    fn doc_tier_supports_named_levels() {
        with_env(None, None, Some("light"), || {
            assert_eq!(bench_doc_counts(), &[100]);
        });
        with_env(None, None, Some("medium"), || {
            assert_eq!(bench_doc_counts(), &[100, 1_000]);
        });
        with_env(None, None, Some("heavy"), || {
            assert_eq!(bench_doc_counts(), &[100, 1_000, 5_000]);
        });
        with_env(None, None, Some("extreme"), || {
            assert_eq!(bench_doc_counts(), &[100, 1_000, 5_000, 10_000]);
        });
    }

    #[test]
    fn doc_tier_invalid_value_falls_back_to_medium() {
        with_env(None, None, Some("banana"), || {
            assert_eq!(bench_doc_counts(), &[100, 1_000]);
        });
    }
}
