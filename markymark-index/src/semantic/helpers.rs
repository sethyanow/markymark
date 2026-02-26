use std::collections::BTreeSet;
use std::path::Path;

use markymark_core::prelude::DocumentUri;

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
    let stale_adjusted = ((top_k as u64 * index_count as u64) / active_count as u64) as u32;
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
