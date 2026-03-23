// Blob-based DocumentIndex tests — mixed documents and XML tags.

use super::super::super::DocumentIndex;
use super::blob_for;

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

// ── XML tag blob-native tests (test 16–17) ───────────────────────────

#[test]
fn test_from_blob_xml_tags_native() {
    // XML tags are now extracted by Zig and stored in the blob directly.
    // Tags must be block-level HTML (own line, blank lines around) to be extracted.
    let text = "# Heading\n\n<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n";
    let blob = blob_for(text);
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    // Headings still work
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Heading");

    // XML tags are populated from blob
    assert!(
        !index.xml_tags().is_empty(),
        "xml_tags should be extracted from blob"
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
fn test_from_blob_xml_tags_self_closing() {
    let text = "\n<br />\n";
    let blob = blob_for(text);
    let index = DocumentIndex::from_blob(&blob).expect("from_blob failed");

    let xml_tags = index.xml_tags();
    assert!(
        !xml_tags.is_empty(),
        "should extract self-closing tag from blob"
    );
    assert_eq!(xml_tags[0].tag_name, "br");
    assert!(xml_tags[0].is_self_closing);
}

