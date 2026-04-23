//! Equivalence tests: `DocumentIndex::from_text()` vs
//! `fallback_scan_with_frontmatter()`.
//!
//! The engine path (from_text) must produce the same index as the scan path
//! (fallback_scan_with_frontmatter) so either can be used as a fallback in
//! MCP and LSP.

use super::*;

/// Verify `DocumentIndex::from_text()` produces equivalent output to
/// `fallback_scan_with_frontmatter()` for a mixed markdown document.
///
/// This equivalence test ensures the engine path (from_text) can safely
/// replace the scan path (fallback_scan_with_frontmatter) as the fallback
/// in both MCP and LSP.
#[test]
fn from_text_equivalence_with_fallback_scan_mixed_doc() {
    use markymark_index::DocumentIndex;

    let text = "\
---
title: Equivalence Test
tags: [alpha, beta]
aliases: [eq1, eq2]
---

# First Heading

Some body with a [[wiki link]] and a [markdown link](http://example.com).

## Second Heading {#custom-id}

A paragraph with `inline code` and <custom-tag>content</custom-tag>.

- [ ] Task one
- [x] Task two

> [!note]
> A callout block.

^block-ref-id
";

    let scan_index = fallback_scan_with_frontmatter(text);
    let engine_index = DocumentIndex::from_text(text);

    // Headings: count and text
    let scan_headings: Vec<(&str, u8)> = scan_index
        .headings()
        .iter()
        .map(|h| (h.text, h.level))
        .collect();
    let engine_headings: Vec<(&str, u8)> = engine_index
        .headings()
        .iter()
        .map(|h| (h.text, h.level))
        .collect();
    assert_eq!(
        scan_headings, engine_headings,
        "headings mismatch: scan={scan_headings:?} vs engine={engine_headings:?}"
    );

    // Tags
    let scan_tags: Vec<&str> = scan_index.tags().iter().map(|t| t.name).collect();
    let engine_tags: Vec<&str> = engine_index.tags().iter().map(|t| t.name).collect();
    assert_eq!(
        scan_tags, engine_tags,
        "tags mismatch: scan={scan_tags:?} vs engine={engine_tags:?}"
    );

    // Wiki links
    let scan_wiki: Vec<&str> = scan_index.wiki_links().iter().map(|w| w.target).collect();
    let engine_wiki: Vec<&str> = engine_index.wiki_links().iter().map(|w| w.target).collect();
    assert_eq!(
        scan_wiki, engine_wiki,
        "wiki links mismatch: scan={scan_wiki:?} vs engine={engine_wiki:?}"
    );

    // Markdown links
    let scan_md_links: Vec<(&str, &str)> = scan_index
        .markdown_links()
        .iter()
        .map(|l| (l.text, l.url))
        .collect();
    let engine_md_links: Vec<(&str, &str)> = engine_index
        .markdown_links()
        .iter()
        .map(|l| (l.text, l.url))
        .collect();
    assert_eq!(
        scan_md_links, engine_md_links,
        "markdown links mismatch: scan={scan_md_links:?} vs engine={engine_md_links:?}"
    );

    // Frontmatter keys
    let scan_fm: Vec<&str> = scan_index.frontmatter().iter().map(|f| f.key).collect();
    let engine_fm: Vec<&str> = engine_index.frontmatter().iter().map(|f| f.key).collect();
    assert_eq!(
        scan_fm, engine_fm,
        "frontmatter keys mismatch: scan={scan_fm:?} vs engine={engine_fm:?}"
    );

    // Aliases
    assert_eq!(
        scan_index.aliases(),
        engine_index.aliases(),
        "aliases mismatch"
    );

    // XML tags
    let scan_xml: Vec<&str> = scan_index.xml_tags().iter().map(|x| x.tag_name).collect();
    let engine_xml: Vec<&str> = engine_index.xml_tags().iter().map(|x| x.tag_name).collect();
    assert_eq!(
        scan_xml, engine_xml,
        "xml tags mismatch: scan={scan_xml:?} vs engine={engine_xml:?}"
    );

    // Tasks
    assert_eq!(
        scan_index.tasks().len(),
        engine_index.tasks().len(),
        "task count mismatch"
    );

    // Code spans
    let scan_code: Vec<&str> = scan_index.code_spans().iter().map(|c| c.text).collect();
    let engine_code: Vec<&str> = engine_index.code_spans().iter().map(|c| c.text).collect();
    assert_eq!(
        scan_code, engine_code,
        "code spans mismatch: scan={scan_code:?} vs engine={engine_code:?}"
    );
}

/// Verify equivalence for a frontmatter-only document (no markdown body).
///
/// Adversarial finding: after mask_frontmatter, the entire text is whitespace.
/// Both paths should produce an index with frontmatter but no headings/links.
#[test]
fn from_text_equivalence_frontmatter_only_doc() {
    use markymark_index::DocumentIndex;

    let text = "---\ntitle: Only Frontmatter\ntags: [solo]\n---\n";

    let scan_index = fallback_scan_with_frontmatter(text);
    let engine_index = DocumentIndex::from_text(text);

    // Frontmatter preserved
    let scan_fm: Vec<&str> = scan_index.frontmatter().iter().map(|f| f.key).collect();
    let engine_fm: Vec<&str> = engine_index.frontmatter().iter().map(|f| f.key).collect();
    assert_eq!(
        scan_fm, engine_fm,
        "frontmatter keys mismatch for frontmatter-only doc"
    );

    // No headings, links, etc.
    assert_eq!(scan_index.headings().len(), engine_index.headings().len());
    assert_eq!(
        scan_index.wiki_links().len(),
        engine_index.wiki_links().len()
    );
    assert_eq!(
        scan_index.markdown_links().len(),
        engine_index.markdown_links().len()
    );
}
