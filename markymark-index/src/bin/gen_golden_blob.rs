//! One-off generator: writes testdata/golden_v1.blob to disk.
//!
//! Run with:
//!   cargo run -p markymark-index --bin gen_golden_blob --features zig-kernels
//!
//! Commit the output file, then delete this binary.

use markymark_kernels::engine::DocumentEngine;
use std::path::PathBuf;

const GOLDEN_MARKDOWN: &str = concat!(
    "# Title One\n\n",
    "## Section A\n\n",
    "## Section A\n\n",
    "[[Simple Link]]\n",
    "[[Page Name|Display Text]]\n",
    "[Click here](https://example.com)\n",
    "[Anchored](doc.md#section)\n",
    "tags: #alpha #beta #gamma\n",
    "block one ^id-one\n",
    "block two ^id-two\n",
);

fn main() {
    let engine = DocumentEngine::new(GOLDEN_MARKDOWN).expect("engine creation failed");
    let blob = engine.get_blob().expect("get_blob failed").data().to_vec();

    let out: PathBuf = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("src/document/testdata/golden_v1.blob");

    std::fs::create_dir_all(out.parent().unwrap()).expect("create testdata dir");
    std::fs::write(&out, &blob).expect("write golden blob");

    println!("Wrote {} bytes → {}", blob.len(), out.display());
}
