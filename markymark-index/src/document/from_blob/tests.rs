use super::super::DocumentIndex;
use super::*;
use markymark_kernels::engine::DocumentEngine;

/// Helper: create a blob from markdown text via the real Zig engine.
fn blob_for(text: &str) -> Vec<u8> {
    let engine = DocumentEngine::new(text).expect("engine creation failed");
    engine.get_blob().expect("get_blob failed").data().to_vec()
}

/// Helper: construct a minimal v1 blob fixture (64-byte header, version=1).
fn make_v1_empty_blob() -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[0..4].copy_from_slice(&BLOB_MAGIC.to_le_bytes());
    buf[4..6].copy_from_slice(&BLOB_VERSION_V1.to_le_bytes());
    buf[44..48].copy_from_slice(&64u32.to_le_bytes()); // total_blob_size
    buf
}

/// Helper: construct a minimal v2 blob fixture (128-byte header, version=2).
fn make_v2_empty_blob() -> [u8; 128] {
    let mut buf = [0u8; 128];
    buf[0..4].copy_from_slice(&BLOB_MAGIC.to_le_bytes());
    buf[4..6].copy_from_slice(&BLOB_VERSION_V2.to_le_bytes());
    buf[44..48].copy_from_slice(&128u32.to_le_bytes()); // total_blob_size
    buf
}

// ── Engine-backed tests (tests 1–9, 14–15) ───────────────────────────

#[test]
fn test_from_blob_empty_document() {
    let blob = blob_for("");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.headings().is_empty());
    assert!(index.wiki_links().is_empty());
    assert!(index.tags().is_empty());
    assert!(index.markdown_links().is_empty());
    assert!(index.toc().is_empty());
    assert_eq!(index.block_ids().count(), 0);
}

#[test]
fn test_from_blob_single_heading() {
    let blob = blob_for("# Hello\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Hello");
    assert_eq!(index.headings()[0].slug, "hello");
    assert_eq!(index.headings()[0].level, 1);
    // Heading should be reachable by slug
    assert!(index.heading_by_slug("hello").is_some());
}

#[test]
fn test_from_blob_multiple_headings_with_dedup_slugs() {
    let blob = blob_for("# Title\n\n# Title\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.headings().len(), 2);
    assert_eq!(index.headings()[0].slug, "title");
    assert_eq!(index.headings()[1].slug, "title-1");
}

#[test]
fn test_from_blob_wiki_link() {
    let blob = blob_for("[[My Page]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    assert_eq!(index.wiki_links()[0].target, "My Page");
    assert_eq!(index.wiki_links()[0].alias, None);
    assert_eq!(index.wiki_links()[0].heading, None);
}

#[test]
fn test_from_blob_wiki_link_with_alias() {
    let blob = blob_for("[[target|display]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    assert_eq!(index.wiki_links()[0].target, "target");
    assert_eq!(index.wiki_links()[0].alias, Some("display"));
}

/// Generic wiki link with heading — verify both target and heading are extracted.
#[test]
fn test_from_blob_wiki_link_with_heading() {
    let blob = blob_for("[[page#heading]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    let wl = &index.wiki_links()[0];
    assert_eq!(wl.target, "page");
    assert_eq!(wl.heading, Some("heading"));
}

/// marky-d7hh: [[page#heading|page]] — alias text matches the page part.
/// from_blob was comparing text != page (anchor-stripped), so "page" == "page"
/// incorrectly produced alias=None. Fix: compare text != target (full target).
#[test]
fn test_from_blob_wiki_link_with_heading_and_matching_alias() {
    let blob = blob_for("[[page#heading|page]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    let wl = &index.wiki_links()[0];
    assert_eq!(
        wl.target, "page",
        "target should be page-only (anchor stripped)"
    );
    assert_eq!(
        wl.heading,
        Some("heading"),
        "heading field should be populated"
    );
    assert_eq!(
        wl.alias,
        Some("page"),
        "alias should be Some when text differs from full target"
    );
}

/// marky-d7hh: [[page#heading|other]] — alias text differs from both page and full target.
/// This case was already correct before the fix; regression guard.
#[test]
fn test_from_blob_wiki_link_with_heading_and_different_alias() {
    let blob = blob_for("[[page#heading|other]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    let wl = &index.wiki_links()[0];
    assert_eq!(wl.target, "page");
    assert_eq!(wl.heading, Some("heading"));
    assert_eq!(wl.alias, Some("other"));
}

/// marky-d7hh: [[page#heading]] — no alias, anchor only.
/// from_blob was comparing text="page#heading" != page="page" → alias=Some("page#heading").
/// Fix: text="page#heading" != target="page#heading" → False → alias=None.
#[test]
fn test_from_blob_wiki_link_with_heading_no_alias() {
    let blob = blob_for("[[page#heading]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.wiki_links().len(), 1);
    let wl = &index.wiki_links()[0];
    assert_eq!(wl.target, "page");
    assert_eq!(wl.heading, Some("heading"));
    assert_eq!(wl.alias, None, "no pipe separator means no alias");
}

#[test]
fn test_from_blob_markdown_link_with_anchor() {
    let blob = blob_for("[text](url.md#frag)\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.markdown_links().len(), 1);
    assert_eq!(index.markdown_links()[0].text, "text");
    assert_eq!(index.markdown_links()[0].url, "url.md");
    assert_eq!(index.markdown_links()[0].anchor, Some("frag"));
}

#[test]
fn test_from_blob_tags() {
    let blob = blob_for("text #alpha #beta\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.tags().len() >= 2);
    assert!(index.tags().iter().any(|t| t.name == "alpha"));
    assert!(index.tags().iter().any(|t| t.name == "beta"));
}

#[test]
fn test_from_blob_block_ids() {
    let blob = blob_for("content ^my-id\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(
        index.block_by_id("my-id").is_some(),
        "block ID 'my-id' should be indexed"
    );
}

#[test]
fn test_from_blob_toc_and_outline() {
    let blob = blob_for("# A\n\n## B\n\n### C\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    let toc = index.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].depth, 0);
    assert_eq!(toc[1].depth, 1);
    assert_eq!(toc[2].depth, 2);

    let outline = index.outline();
    assert_eq!(outline.children.len(), 1, "root has 1 L1 child");
    assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "A");
    assert_eq!(outline.children[0].children.len(), 1, "A has 1 L2 child");
}

// ── Validation rejection tests (tests 10–13) ─────────────────────────

#[test]
fn test_from_blob_rejects_invalid_magic() {
    let mut buf = [0u8; 64];
    // Write wrong magic in little-endian
    buf[0] = 0xEF;
    buf[1] = 0xBE;
    buf[2] = 0xAD;
    buf[3] = 0xDE;
    // Write valid BLOB_VERSION (1)
    buf[4] = 1;
    buf[5] = 0;
    // total_blob_size = 64
    buf[44] = 64;
    assert!(matches!(
        DocumentIndex::from_blob(&buf),
        Err(BlobError::InvalidMagic)
    ));
}

#[test]
fn test_from_blob_rejects_bad_version() {
    let mut buf = [0u8; 64];
    // Write correct magic: 0x4D4B5343 in little-endian
    buf[0] = 0x43;
    buf[1] = 0x53;
    buf[2] = 0x4B;
    buf[3] = 0x4D;
    // Write version = 99
    buf[4] = 99;
    buf[5] = 0;
    assert!(matches!(
        DocumentIndex::from_blob(&buf),
        Err(BlobError::UnsupportedVersion)
    ));
}

#[test]
fn test_from_blob_rejects_truncated() {
    let buf = [0u8; 32];
    assert!(matches!(
        DocumentIndex::from_blob(&buf),
        Err(BlobError::TooSmall)
    ));
}

#[test]
fn test_from_blob_v2_empty_document() {
    let buf = make_v2_empty_blob();
    let index = DocumentIndex::from_blob(&buf).expect("v2 empty blob should parse");
    assert!(index.headings().is_empty());
    assert!(index.wiki_links().is_empty());
    assert!(index.tags().is_empty());
}

#[test]
fn test_from_blob_rejects_truncated_v2() {
    // 80 bytes: enough for v1 header but too small for v2.
    let mut buf = [0u8; 80];
    buf[0..4].copy_from_slice(&BLOB_MAGIC.to_le_bytes());
    buf[4..6].copy_from_slice(&BLOB_VERSION_V2.to_le_bytes());
    assert!(matches!(
        DocumentIndex::from_blob(&buf),
        Err(BlobError::TooSmall)
    ));
}

#[test]
fn test_from_blob_v2_same_result_as_v1_for_empty_doc() {
    let v1_blob = make_v1_empty_blob();
    let v2_blob = make_v2_empty_blob();

    let v1_index = DocumentIndex::from_blob(&v1_blob).expect("v1 empty blob should parse");
    let v2_index =
        DocumentIndex::from_blob(&v2_blob).expect("v2 empty blob should parse equivalently");

    assert_eq!(v1_index.headings().len(), v2_index.headings().len());
    assert_eq!(v1_index.wiki_links().len(), v2_index.wiki_links().len());
    assert_eq!(
        v1_index.markdown_links().len(),
        v2_index.markdown_links().len()
    );
    assert_eq!(v1_index.tags().len(), v2_index.tags().len());
    assert_eq!(v1_index.block_ids().count(), v2_index.block_ids().count());
}

#[test]
fn test_from_blob_rejects_size_mismatch() {
    // Build a valid minimal blob (header only) but corrupt total_blob_size.
    let blob = blob_for("");
    assert_eq!(blob.len(), 128);
    let mut corrupt = blob.clone();
    // Set total_blob_size to 64 (doesn't match actual 128 bytes)
    corrupt[44] = 64;
    corrupt[45] = 0;
    corrupt[46] = 0;
    corrupt[47] = 0;
    assert!(matches!(
        DocumentIndex::from_blob(&corrupt),
        Err(BlobError::SizeMismatch)
    ));
}

// ── Parity test (test 14) ─────────────────────────────────────────────

#[test]
fn test_from_blob_parity_with_from_scan() {
    // Compare blob (DocumentEngine/md4c) vs from_scan with Md4cScanBackend.
    // Both use md4c extraction so offsets are identical.
    // ZigScanBackend uses SIMD scanner with different offset conventions.
    use markymark_core::scanner::Md4cScanBackend;

    let text =
        "# Main Heading\n\n## Sub Heading\n\n[[Wiki Link]]\n[[Page|Alias]]\n[md](url.md#sec)\n#tag1 #tag2\ncontent ^block1\n";

    // Build via engine blob path
    let blob = blob_for(text);
    let blob_idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    // Build via md4c scan path (same extraction as DocumentEngine)
    let backend = Md4cScanBackend;
    let scan_idx = DocumentIndex::from_scan(text, &backend);

    // Headings: text, slug, level, range must match exactly
    let blob_headings = blob_idx.headings();
    let scan_headings = scan_idx.headings();
    assert_eq!(blob_headings.len(), scan_headings.len(), "heading count");
    for (b, s) in blob_headings.iter().zip(scan_headings.iter()) {
        assert_eq!(b.text, s.text, "heading text");
        assert_eq!(b.slug, s.slug, "heading slug");
        assert_eq!(b.level, s.level, "heading level");
        assert_eq!(b.range, s.range, "heading range for '{}'", b.text);
    }

    // Wiki links: target, alias, range must match
    let blob_wl = blob_idx.wiki_links();
    let scan_wl = scan_idx.wiki_links();
    assert_eq!(blob_wl.len(), scan_wl.len(), "wiki link count");
    for (b, s) in blob_wl.iter().zip(scan_wl.iter()) {
        assert_eq!(b.target, s.target, "wiki link target");
        assert_eq!(b.alias, s.alias, "wiki link alias");
        assert_eq!(b.range, s.range, "wiki link range for '{}'", b.target);
    }

    // Markdown links: text, url, anchor, range must match
    let blob_ml = blob_idx.markdown_links();
    let scan_ml = scan_idx.markdown_links();
    assert_eq!(blob_ml.len(), scan_ml.len(), "markdown link count");
    for (b, s) in blob_ml.iter().zip(scan_ml.iter()) {
        assert_eq!(b.text, s.text, "markdown link text");
        assert_eq!(b.url, s.url, "markdown link url");
        assert_eq!(b.anchor, s.anchor, "markdown link anchor");
        assert_eq!(b.range, s.range, "markdown link range for '{}'", b.text);
    }

    // Tags: names must match (order may differ — use set comparison)
    let blob_tags: std::collections::HashSet<&str> =
        blob_idx.tags().iter().map(|t| t.name).collect();
    let scan_tags: std::collections::HashSet<&str> =
        scan_idx.tags().iter().map(|t| t.name).collect();
    assert_eq!(blob_tags, scan_tags, "tag names");

    // Block IDs: must match
    let blob_blocks: std::collections::HashSet<&str> = blob_idx.block_ids().collect();
    let scan_blocks: std::collections::HashSet<&str> = scan_idx.block_ids().collect();
    assert_eq!(blob_blocks, scan_blocks, "block IDs");
}

// ── Mixed document test (test 15) ─────────────────────────────────────

#[test]
fn test_from_blob_mixed_document() {
    let text = concat!(
        "# Title One\n\n",
        "## Section A\n\n",
        "## Section A\n\n", // duplicate slug → dedup
        "[[Simple Link]]\n",
        "[[Page Name|Display Text]]\n",
        "[Click here](https://example.com)\n",
        "[Anchored](doc.md#section)\n",
        "tags: #alpha #beta #gamma\n",
        "block one ^id-one\n",
        "block two ^id-two\n",
    );
    let blob = blob_for(text);
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    // Headings with deduplication
    assert_eq!(index.headings().len(), 3);
    assert_eq!(index.headings()[0].slug, "title-one");
    assert_eq!(index.headings()[1].slug, "section-a");
    assert_eq!(index.headings()[2].slug, "section-a-1");

    // Wiki links
    assert_eq!(index.wiki_links().len(), 2);
    assert!(index
        .wiki_links()
        .iter()
        .any(|w| w.target == "Simple Link" && w.alias.is_none()));
    assert!(index
        .wiki_links()
        .iter()
        .any(|w| w.target == "Page Name" && w.alias == Some("Display Text")));

    // Markdown links
    assert_eq!(index.markdown_links().len(), 2);
    assert!(index
        .markdown_links()
        .iter()
        .any(|m| m.url == "https://example.com" && m.anchor.is_none()));
    assert!(index
        .markdown_links()
        .iter()
        .any(|m| m.url == "doc.md" && m.anchor == Some("section")));

    // Tags
    assert!(index.tags().iter().any(|t| t.name == "alpha"));
    assert!(index.tags().iter().any(|t| t.name == "beta"));
    assert!(index.tags().iter().any(|t| t.name == "gamma"));

    // Block IDs
    assert!(index.block_by_id("id-one").is_some());
    assert!(index.block_by_id("id-two").is_some());

    // TOC
    let toc = index.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].depth, 0);
    assert_eq!(toc[1].depth, 1);
    assert_eq!(toc[2].depth, 1);
}

// ── XML tag supplementary tests (test 16–17) ─────────────────────────

#[test]
fn test_from_blob_with_xml_tags() {
    let text = "# Heading\n\n<agent>content</agent>\n\n<goal>win</goal>\n";
    let blob = blob_for(text);
    let xml_tags = super::extract_xml_tags_from_text(text);

    assert!(xml_tags.len() >= 2, "should extract agent and goal tags");

    let index =
        DocumentIndex::from_blob_with_xml_tags(&blob, xml_tags).expect("from_blob failed");

    // Headings still work
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Heading");

    // XML tags are populated
    assert!(
        !index.xml_tags().is_empty(),
        "xml_tags should not be empty when provided"
    );
    let tag_names: Vec<&str> = index.xml_tags().iter().map(|xt| xt.tag_name).collect();
    assert!(
        tag_names.contains(&"agent"),
        "should include 'agent'; got: {:?}",
        tag_names
    );
    assert!(
        tag_names.contains(&"goal"),
        "should include 'goal'; got: {:?}",
        tag_names
    );
}

#[test]
fn test_extract_xml_tags_from_text_basic() {
    let text = "<agent>hello</agent>\n<goal>win</goal>\n<routing>path</routing>\n";
    let tags = super::extract_xml_tags_from_text(text);
    let names: Vec<&str> = tags.iter().map(|t| t.tag_name.as_str()).collect();
    assert!(
        names.contains(&"agent"),
        "should find agent; got: {:?}",
        names
    );
    assert!(
        names.contains(&"goal"),
        "should find goal; got: {:?}",
        names
    );
    assert!(
        names.contains(&"routing"),
        "should find routing; got: {:?}",
        names
    );
}

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
    let blob = include_bytes!("../testdata/golden_v1.blob");

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

// ── Code span tests (marky-vsh2) ────────────────────────────────────

#[test]
fn test_from_blob_code_spans_basic() {
    let blob = blob_for("Hello `world` end");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "world");
    assert_eq!(cs[0].start_byte, 6); // offset of opening backtick
    assert_eq!(cs[0].end_byte, 13); // past closing backtick
}

#[test]
fn test_from_blob_code_spans_multiple() {
    let blob = blob_for("`a` and `b`");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].text, "a");
    assert_eq!(cs[1].text, "b");
    assert!(cs[1].start_byte > cs[0].start_byte);
}

#[test]
fn test_from_blob_code_spans_empty() {
    let blob = blob_for("No code here");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.code_spans().is_empty());
}

#[test]
fn test_from_blob_code_spans_in_heading() {
    let blob = blob_for("# Title `code` end");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.headings().len(), 1);
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "code");
}

#[test]
fn test_from_blob_code_spans_backward_compat() {
    // A true v1 blob with 1 heading but no code spans.
    // v1 has code_span_count=0 by default (offset 48 is zero-initialized).
    let pool = b"a heading\0a-heading\0\0\0";
    let pool_size = pool.len() as u32;
    let total_size = V1_HEADER_SIZE as u32 + HEADING_SIZE as u32 + pool_size;

    let mut v1_blob = vec![0u8; total_size as usize];
    // Start from the empty v1 header template.
    v1_blob[..V1_HEADER_SIZE].copy_from_slice(&make_v1_empty_blob());
    // Patch counts and sizes.
    v1_blob[16..20].copy_from_slice(&1u32.to_le_bytes()); // heading_count
    v1_blob[36..40].copy_from_slice(&pool_size.to_le_bytes()); // text_pool_size
    v1_blob[44..48].copy_from_slice(&total_size.to_le_bytes()); // total_blob_size

    // BlobHeading: text_off=0, text_len=9, slug_off=10, slug_len=9, level=1
    let h = V1_HEADER_SIZE;
    v1_blob[h..h + 4].copy_from_slice(&0u32.to_le_bytes());
    v1_blob[h + 4..h + 8].copy_from_slice(&9u32.to_le_bytes());
    v1_blob[h + 8..h + 12].copy_from_slice(&10u32.to_le_bytes());
    v1_blob[h + 12..h + 16].copy_from_slice(&9u32.to_le_bytes());
    v1_blob[h + 36] = 1; // level

    // Text pool follows the heading section.
    let pool_start = V1_HEADER_SIZE + HEADING_SIZE;
    v1_blob[pool_start..pool_start + pool.len()].copy_from_slice(pool);

    let index = DocumentIndex::from_blob(&v1_blob).expect("v1 blob should parse");
    assert!(index.code_spans().is_empty());
    assert_eq!(index.headings().len(), 1);
}

#[test]
fn test_from_blob_code_span_positions() {
    let blob = blob_for("line1\n`code`\nline3");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let cs = index.code_spans();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "code");
    // Code span is on line 1 (0-indexed), col 0
    assert_eq!(cs[0].range.start.line, 1);
    assert_eq!(cs[0].range.start.character, 0);
}

#[test]
fn test_from_blob_code_span_parity_with_from_scan() {
    use markymark_core::scanner::Md4cScanBackend;

    let text = "# Heading\n\nSome `code` and `more code` here.\n\n[link](url)";
    let blob = blob_for(text);
    let blob_index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    let backend = Md4cScanBackend;
    let scan_index = DocumentIndex::from_scan(text, &backend);

    let blob_cs = blob_index.code_spans();
    let scan_cs = scan_index.code_spans();
    assert_eq!(blob_cs.len(), scan_cs.len(), "code span count mismatch");
    for (b, s) in blob_cs.iter().zip(scan_cs.iter()) {
        assert_eq!(b.text, s.text, "code span text mismatch");
        assert_eq!(b.start_byte, s.start_byte, "start_byte mismatch");
        assert_eq!(b.end_byte, s.end_byte, "end_byte mismatch");
        assert_eq!(
            b.range.start.line, s.range.start.line,
            "start line mismatch"
        );
        assert_eq!(
            b.range.start.character, s.range.start.character,
            "start col mismatch"
        );
    }
}

// ── Task/Embed from_blob tests (marky-bmu9) ──────────────────────────

#[test]
fn test_from_blob_v2_with_tasks() {
    let blob = blob_for("- [ ] Todo\n- [x] Done\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let tasks = index.tasks();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].state, "unchecked");
    assert_eq!(tasks[0].text, "Todo");
    assert_eq!(tasks[1].state, "checked");
    assert_eq!(tasks[1].text, "Done");
}

#[test]
fn test_from_blob_v2_with_embeds() {
    let blob = blob_for("![[target]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let embeds = index.embeds();
    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].target, "target");
}

#[test]
fn test_from_blob_v2_tasks_and_embeds() {
    let blob = blob_for("- [x] Task\n\n![[embed]]\n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert_eq!(index.tasks().len(), 1);
    assert_eq!(index.embeds().len(), 1);
}

#[test]
fn test_from_blob_v1_no_tasks_or_embeds() {
    let blob = make_v1_empty_blob();
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    assert!(index.tasks().is_empty());
    assert!(index.embeds().is_empty());
}

#[test]
fn test_from_blob_v2_empty_task_text() {
    // A task list item with no text content: `- [ ] \n`
    let blob = blob_for("- [ ] \n");
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    // May or may not have a task depending on engine behavior; just verify no panic
    let _ = index.tasks();
}
