// Test module for from_blob — split into thematic submodules.
// Helpers and re-exports live here; actual tests in child modules.

use super::header::{BLOB_MAGIC, BLOB_VERSION_V1, BLOB_VERSION_V2};
use markymark_kernels::engine::DocumentEngine;

/// Helper: create a blob from markdown text via the real Zig engine.
pub fn blob_for(text: &str) -> Vec<u8> {
    let engine = DocumentEngine::new(text).expect("engine creation failed");
    engine.get_blob().expect("get_blob failed").data().to_vec()
}

/// Helper: construct a minimal v1 blob fixture (64-byte header, version=1).
pub fn make_v1_empty_blob() -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[0..4].copy_from_slice(&BLOB_MAGIC.to_le_bytes());
    buf[4..6].copy_from_slice(&BLOB_VERSION_V1.to_le_bytes());
    buf[44..48].copy_from_slice(&64u32.to_le_bytes()); // total_blob_size
    buf
}

/// Helper: construct a minimal v2 blob fixture (128-byte header, version=2).
pub fn make_v2_empty_blob() -> [u8; 128] {
    let mut buf = [0u8; 128];
    buf[0..4].copy_from_slice(&BLOB_MAGIC.to_le_bytes());
    buf[4..6].copy_from_slice(&BLOB_VERSION_V2.to_le_bytes());
    buf[44..48].copy_from_slice(&128u32.to_le_bytes()); // total_blob_size
    buf
}

mod core_tests;
mod feature_tests;
mod golden_tests;
mod parity_tests;
