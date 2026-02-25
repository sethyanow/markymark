// Core from_blob tests — basic engine-backed tests and validation rejection tests.

use super::super::super::{DocumentIndex, FrontmatterOwnedEntry, FrontmatterValueOwned};
use super::super::header::BlobError;
use super::super::helpers::mask_frontmatter;
use super::{blob_for, make_v1_empty_blob, make_v2_empty_blob};

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
    assert_eq!(index.tags().len(), 2);
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
    use super::super::header::{BLOB_MAGIC, BLOB_VERSION_V2};
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

// ── from_blob_with_frontmatter tests ─────────────────────────────────

#[test]
fn test_from_blob_with_frontmatter_populates_entries() {
    let text = "---\ntitle: Hello\ntags: [a, b]\naliases: [hi, hey]\n---\n# Heading\n";
    // Mask frontmatter before passing to engine, so `---` delimiters are not
    // misparsed as setext headings. This matches the LSP layer's behavior.
    let masked = mask_frontmatter(text);
    let blob = blob_for(&masked);
    let frontmatter = vec![
        FrontmatterOwnedEntry {
            key: "title".to_string(),
            value: FrontmatterValueOwned::String("Hello".to_string()),
        },
        FrontmatterOwnedEntry {
            key: "tags".to_string(),
            value: FrontmatterValueOwned::List(vec!["a".to_string(), "b".to_string()]),
        },
    ];
    let aliases = vec!["hi".to_string(), "hey".to_string()];

    let index = DocumentIndex::from_blob_with_frontmatter(&blob, frontmatter, aliases)
        .expect("from_blob_with_frontmatter failed");

    // Frontmatter entries populated
    assert_eq!(
        index.frontmatter().len(),
        2,
        "should have 2 frontmatter entries"
    );
    assert_eq!(index.frontmatter()[0].key, "title");
    assert_eq!(index.frontmatter()[1].key, "tags");

    // Aliases populated
    assert_eq!(index.aliases().len(), 2, "should have 2 aliases");
    assert!(index.aliases().contains(&"hi"), "missing alias 'hi'");
    assert!(index.aliases().contains(&"hey"), "missing alias 'hey'");

    // Heading from blob still works
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Heading");
}

#[test]
fn test_from_blob_with_frontmatter_empty_fm_still_works() {
    let blob = blob_for("# Just a heading\n");
    let index = DocumentIndex::from_blob_with_frontmatter(&blob, vec![], vec![])
        .expect("from_blob_with_frontmatter with empty fm failed");

    assert!(index.frontmatter().is_empty());
    assert!(index.aliases().is_empty());
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Just a heading");
}
