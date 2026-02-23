// Golden blob roundtrip and BlobError display tests.

use super::super::super::DocumentIndex;
use super::super::header::{validate_blob, BlobError};
use super::blob_for;

// ── Golden blob roundtrip test ────────────────────────────────────────

/// Canonical markdown input used for the golden blob.
///
/// Covers all element types: headings (with slug dedup), wiki links
/// (plain and aliased), markdown links (plain and anchored), tags, and
/// block IDs.
///
/// Generated blob is committed at testdata/golden_v1.blob.
/// Blob version: 1  |  Magic: 0x4D4B5343 ("MKSC")
///
/// To regenerate: `cargo test -p markymark-index generate_golden_blob -- --include-ignored`
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

/// One-off generator — run with:
///   cargo test -p markymark-index generate_golden_blob -- --include-ignored
#[test]
#[ignore]
fn generate_golden_blob() {
    let blob = blob_for(GOLDEN_MARKDOWN);
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let path = std::path::Path::new(&manifest_dir).join("src/document/testdata/golden_v1.blob");
    std::fs::write(&path, &blob).expect("failed to write golden blob");
    println!("Wrote {} bytes to {}", blob.len(), path.display());
}

#[test]
fn test_golden_blob_roundtrip() {
    let blob = include_bytes!("../../testdata/golden_v1.blob");

    // validate_blob() must succeed and header counts must match the input
    let header = validate_blob(blob).expect("validate_blob failed on golden blob");
    assert_eq!(header.heading_count, 3, "expected 3 headings");
    assert_eq!(
        header.link_count, 4,
        "expected 4 links (2 wiki + 2 markdown)"
    );
    assert_eq!(header.tag_count, 3, "expected 3 tags");
    assert_eq!(header.block_id_count, 2, "expected 2 block IDs");

    // from_blob() must succeed
    let index = DocumentIndex::from_blob(blob).expect("from_blob failed on golden blob");

    // Headings: dedup slug check
    assert_eq!(index.headings().len(), 3);
    assert_eq!(index.headings()[0].text, "Title One");
    assert_eq!(index.headings()[0].slug, "title-one");
    assert_eq!(index.headings()[0].level, 1);
    assert_eq!(index.headings()[1].text, "Section A");
    assert_eq!(index.headings()[1].slug, "section-a");
    assert_eq!(index.headings()[1].level, 2);
    assert_eq!(index.headings()[2].slug, "section-a-1");

    // Wiki links
    assert_eq!(index.wiki_links().len(), 2);
    assert!(
        index
            .wiki_links()
            .iter()
            .any(|w| w.target == "Simple Link" && w.alias.is_none()),
        "expected wiki link to 'Simple Link'"
    );
    assert!(
        index
            .wiki_links()
            .iter()
            .any(|w| w.target == "Page Name" && w.alias == Some("Display Text")),
        "expected aliased wiki link to 'Page Name'"
    );

    // Markdown links
    assert_eq!(index.markdown_links().len(), 2);
    assert!(
        index
            .markdown_links()
            .iter()
            .any(|m| m.url == "https://example.com" && m.anchor.is_none()),
        "expected markdown link to https://example.com"
    );
    assert!(
        index
            .markdown_links()
            .iter()
            .any(|m| m.url == "doc.md" && m.anchor == Some("section")),
        "expected anchored markdown link to doc.md#section"
    );

    // Tags
    assert!(
        index.tags().iter().any(|t| t.name == "alpha"),
        "expected tag 'alpha'"
    );
    assert!(
        index.tags().iter().any(|t| t.name == "beta"),
        "expected tag 'beta'"
    );
    assert!(
        index.tags().iter().any(|t| t.name == "gamma"),
        "expected tag 'gamma'"
    );

    // Block IDs
    assert!(
        index.block_by_id("id-one").is_some(),
        "expected block id 'id-one'"
    );
    assert!(
        index.block_by_id("id-two").is_some(),
        "expected block id 'id-two'"
    );
}

#[test]
fn test_blob_error_display_messages() {
    // Each variant must produce a non-empty, distinct human-readable message.
    let cases: &[(BlobError, &str)] = &[
        (BlobError::TooSmall, "64 bytes"),
        (BlobError::InvalidMagic, "MKSC"),
        (BlobError::UnsupportedVersion, "versions 1 and 2"),
        (BlobError::SizeMismatch, "size mismatch"),
        (BlobError::TextPoolOutOfBounds, "text pool"),
        (BlobError::InvalidUtf8, "UTF-8"),
    ];
    for (err, expected_substr) in cases {
        let msg = format!("{}", err);
        assert!(
            msg.contains(expected_substr),
            "Display for {err:?} = {msg:?}; expected to contain {expected_substr:?}"
        );
    }
}

#[test]
fn test_blob_error_is_std_error() {
    // BlobError must be usable as Box<dyn std::error::Error>.
    fn accepts_error(_: &dyn std::error::Error) {}
    accepts_error(&BlobError::InvalidMagic);

    // Must be usable with ? in Box<dyn Error> context.
    fn returns_box_err() -> Result<(), Box<dyn std::error::Error>> {
        let data: &[u8] = &[0u8; 4]; // too small
        DocumentIndex::from_blob(data)?; // should propagate BlobError::TooSmall
        Ok(())
    }
    assert!(returns_box_err().is_err());
}

#[test]
fn test_blob_error_display_all_variants_distinct() {
    // All 6 variant messages must be distinct (catch copy-paste errors).
    use std::collections::HashSet;
    let messages: HashSet<String> = [
        BlobError::TooSmall,
        BlobError::InvalidMagic,
        BlobError::UnsupportedVersion,
        BlobError::SizeMismatch,
        BlobError::TextPoolOutOfBounds,
        BlobError::InvalidUtf8,
    ]
    .iter()
    .map(|e| format!("{e}"))
    .collect();
    assert_eq!(
        messages.len(),
        6,
        "All BlobError variants must have distinct Display messages"
    );
}
