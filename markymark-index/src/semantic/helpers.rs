use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use markymark_core::prelude::DocumentUri;

use crate::DocumentIndex;

const FETCH_OVERFETCH_MULTIPLIER: u32 = 4;

pub(super) fn fallback_heading(uri: &DocumentUri) -> String {
    uri.to_file_path()
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(uri.as_str())
        .to_string()
}

pub(super) fn token_hashes(text: &str) -> Vec<u32> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| fnv1a32(&token.to_ascii_lowercase()))
        .collect()
}

pub(super) fn compute_fetch_k(index_count: u32, active_count: u32, top_k: u32) -> u32 {
    if index_count == 0 || top_k == 0 || active_count == 0 {
        return 0;
    }
    // Scale fetch size by stale ratio so enough active entries survive filtering.
    // If 80% of vectors are stale, we need ~5x raw hits per desired result.
    let stale_adjusted =
        ((top_k as u64 * index_count as u64) / active_count as u64).min(u32::MAX as u64) as u32;
    let baseline = top_k.saturating_mul(FETCH_OVERFETCH_MULTIPLIER);
    let needed = stale_adjusted.max(baseline);
    index_count.min(needed)
}

pub(super) fn fnv1a32(text: &str) -> u32 {
    const OFFSET: u32 = 0x811c9dc5;
    const PRIME: u32 = 0x0100_0193;

    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

pub(super) fn jaccard_similarity(a: &BTreeSet<u32>, b: &BTreeSet<u32>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }

    let intersection = a.intersection(b).count() as f32;
    let union = (a.len() + b.len()) as f32 - intersection;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Group content blocks by parent heading and concatenate their text.
///
/// Returns a map from heading index (or `None` for blocks before any heading)
/// to concatenated block text. Empty blocks are skipped.
pub(super) fn section_block_texts(index: &DocumentIndex) -> HashMap<Option<usize>, String> {
    let blocks = index.content_blocks();
    if blocks.is_empty() {
        return HashMap::new();
    }

    let mut sections: HashMap<Option<usize>, String> = HashMap::new();
    for block in blocks {
        let text = index.block_text(block).trim();
        if text.is_empty() {
            continue;
        }
        let entry = sections.entry(block.parent_heading).or_default();
        if !entry.is_empty() {
            entry.push('\n');
        }
        entry.push_str(text);
    }
    sections
}

/// Build the embedding input for a section: heading text + block text (if any).
pub(super) fn build_embedding_input(heading_text: &str, block_text: Option<&str>) -> String {
    match block_text {
        Some(bt) if !bt.is_empty() => {
            let mut input = String::with_capacity(heading_text.len() + 1 + bt.len());
            input.push_str(heading_text);
            input.push('\n');
            input.push_str(bt);
            input
        }
        _ => heading_text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: stale_adjusted for (u32::MAX, 1, 2) is ~8.6 billion without the clamp,
    /// which would truncate to a small value. With the clamp it saturates at u32::MAX,
    /// and index_count.min(needed) then caps at u32::MAX.
    #[test]
    fn test_compute_fetch_k_large_index_no_truncation() {
        let result = compute_fetch_k(u32::MAX, 1, 2);
        assert_eq!(result, u32::MAX);
    }

    /// Regression: stale_adjusted for (1_000_000, 1, 5_000) is 5_000_000_000 > u32::MAX.
    /// Without the clamp this wraps to a small value, causing under-fetching.
    /// With the clamp it saturates at u32::MAX, then index_count caps the result at 1_000_000.
    #[test]
    fn test_compute_fetch_k_extreme_stale_ratio() {
        let result = compute_fetch_k(1_000_000, 1, 5_000);
        assert_eq!(result, 1_000_000);
    }

    /// Sanity-check for a normal (non-overflowing) case.
    /// stale_adjusted = (10 * 1000) / 500 = 20
    /// baseline = 10 * FETCH_OVERFETCH_MULTIPLIER (4) = 40
    /// needed = max(20, 40) = 40
    /// result = min(1000, 40) = 40
    #[test]
    fn test_compute_fetch_k_normal_values() {
        let result = compute_fetch_k(1000, 500, 10);
        assert_eq!(result, 40);
    }
}
