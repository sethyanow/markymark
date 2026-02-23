// Parity tests — from_blob vs from_scan comparison, mixed documents, and XML tags.

use super::{blob_for, extract_xml_tags_from_text, DocumentIndex};

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
    let xml_tags = extract_xml_tags_from_text(text);

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
    let tags = extract_xml_tags_from_text(text);
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
