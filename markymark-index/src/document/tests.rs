//! Document index tests.

use super::*;
use bumpalo::Bump;
use hashbrown::HashMap;
use markymark_core::prelude::*;
use markymark_parser::Parser;

fn build_index(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    DocumentIndex::from_ast(ast)
}

#[test]
fn heading_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = HeadingEntry {
        text: arena.alloc_str("Intro"),
        slug: arena.alloc_str("intro"),
        level: 1,
        range: Range::new(Position::new(0, 0), Position::new(0, 5)),
    };

    assert_eq!(entry.text, "Intro");
    assert_eq!(entry.slug, "intro");
    assert_eq!(entry.level, 1);
}

#[test]
fn block_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = BlockEntry {
        id: arena.alloc_str("block-1"),
        range: Range::new(Position::new(0, 0), Position::new(0, 7)),
    };

    assert_eq!(entry.id, "block-1");
}

#[test]
fn toc_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = TocEntry {
        text: arena.alloc_str("Section"),
        slug: arena.alloc_str("section"),
        level: 2,
        depth: 1,
    };

    assert_eq!(entry.text, "Section");
    assert_eq!(entry.slug, "section");
    assert_eq!(entry.depth, 1);
}

#[test]
fn outline_node_uses_arena_lifetime() {
    let root = OutlineNode {
        heading: None,
        children: &[],
    };

    assert!(root.heading.is_none());
    assert!(root.children.is_empty());
}

#[test]
fn wiki_link_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = WikiLinkEntry {
        target: arena.alloc_str("TargetPage"),
        alias: Some(arena.alloc_str("Alias")),
        heading: Some(arena.alloc_str("Section")),
        range: Range::new(Position::new(0, 0), Position::new(0, 10)),
        start_byte: 0,
        end_byte: 10,
    };

    assert_eq!(entry.target, "TargetPage");
    assert_eq!(entry.alias, Some("Alias"));
    assert_eq!(entry.heading, Some("Section"));
}

#[test]
fn tag_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = TagEntry {
        name: arena.alloc_str("project/feature"),
    };

    assert_eq!(entry.name, "project/feature");
}

#[test]
fn markdown_link_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let entry = MarkdownLinkEntry {
        text: arena.alloc_str("Example"),
        url: arena.alloc_str("https://example.com"),
        anchor: Some(arena.alloc_str("a")),
        range: Range::new(Position::new(0, 0), Position::new(0, 7)),
    };

    assert_eq!(entry.text, "Example");
    assert_eq!(entry.url, "https://example.com");
    assert_eq!(entry.anchor, Some("a"));
}

#[test]
fn xml_tag_entry_uses_arena_lifetime() {
    let arena = Bump::new();
    let mut attrs = HashMap::new();
    let priority: &str = arena.alloc_str("priority");
    let high: &str = arena.alloc_str("high");
    attrs.insert(priority, high);

    let entry = XmlTagEntry {
        tag_name: arena.alloc_str("goal"),
        attributes: attrs,
        is_self_closing: false,
        is_unclosed: false,
        range: Range::new(Position::new(0, 0), Position::new(0, 6)),
    };

    assert_eq!(entry.tag_name, "goal");
    assert_eq!(entry.attributes.get("priority"), Some(&"high"));
}

#[test]
fn document_index_uses_arena_lifetime() {
    let index = build_index("# Root\n\n## Child\n");

    assert_eq!(index.headings().len(), 2);
    assert_eq!(index.headings()[0].text, "Root");
    assert_eq!(index.headings()[1].slug, "child");
}

#[test]
fn document_index_uses_hashbrown_with_arena() {
    let index = build_index("# Root\n\nA block ^block-1\n");

    // HashMap-backed lookups should work for arena-allocated keys.
    assert!(index.heading_by_slug("root").is_some());
    assert!(index.block_by_id("block-1").is_some());
}

#[test]
fn xml_tag_entry_attributes_arena_map() {
    let index = build_index("<goal priority=\"high\" status=\"open\">Ship</goal>\n");

    let tags = index.xml_tags();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].attributes.get("priority"), Some(&"high"));
    assert_eq!(tags[0].attributes.get("status"), Some(&"open"));
}

#[test]
fn document_index_vecs_become_slices() {
    let index = build_index("# A\n\n## B\n\n[[Page]]\n#tag\n");

    let _: &[HeadingEntry<'_>] = index.headings();
    let _: &[TocEntry<'_>] = index.toc();
    let _: &[WikiLinkEntry<'_>] = index.wiki_links();
    let _: &[TagEntry<'_>] = index.tags();
    let _: &[MarkdownLinkEntry<'_>] = index.markdown_links();
    let _: &[XmlTagEntry<'_>] = index.xml_tags();

    assert!(!index.headings().is_empty());
    assert!(!index.toc().is_empty());
}

#[test]
fn outline_node_children_arena_slice() {
    let index = build_index("# Root\n\n## Child\n\n### Grandchild\n");

    let outline = index.outline();
    assert_eq!(outline.children.len(), 1);
    assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
    assert_eq!(outline.children[0].children.len(), 1);
}

#[test]
fn from_ast_propagates_arena_lifetime() {
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse("# Arena\n").unwrap();
    let index = DocumentIndex::from_ast(ast);

    let heading: &HeadingEntry<'_> = &index.headings()[0];
    assert_eq!(heading.text, "Arena");
}

#[test]
fn heading_by_slug_returns_arena_ref() {
    let index = build_index("# Root\n\n## Root\n");

    let heading = index.heading_by_slug("root").unwrap();
    let _: &HeadingEntry<'_> = heading;
    assert_eq!(heading.text, "Root");
}

#[test]
fn toc_returns_arena_slice() {
    let index = build_index("# A\n\n## B\n\n### C\n");

    let toc: &[TocEntry<'_>] = index.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].depth, 0);
    assert_eq!(toc[1].depth, 1);
    assert_eq!(toc[2].depth, 2);
}

#[test]
fn parser_types_flow_to_index() {
    let index =
        build_index("# Heading\n\n[[Page#section]]\n#tag\n[Link](https://example.com#frag)\n");

    assert_eq!(index.headings()[0].text, "Heading");
    assert_eq!(index.wiki_links()[0].target, "Page");
    assert_eq!(index.wiki_links()[0].heading, Some("section"));

    // Tag extraction from this fixture can include heading anchors as tags;
    // assert that our expected tag is present rather than position-dependent.
    assert!(index.tags().iter().any(|t| t.name == "tag"));

    assert_eq!(index.markdown_links()[0].anchor, Some("frag"));
}

#[test]
fn document_index_to_realm_integration() {
    let index_a = build_index("# Doc A\n\nA block ^a\n");
    let index_b = build_index("# Doc B\n\nA block ^b\n");

    assert_eq!(index_a.headings()[0].text, "Doc A");
    assert_eq!(index_b.headings()[0].text, "Doc B");
    assert!(index_a.block_by_id("a").is_some());
    assert!(index_b.block_by_id("b").is_some());
}

// ---------------------------------------------------------------------------
// Scan-based construction tests (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "zig-kernels")]
mod scan_tests {
    use super::*;
    use markymark_core::scanner::ZigScanBackend;

    fn build_index_from_scan(source: &str) -> DocumentIndex {
        let backend = ZigScanBackend;
        DocumentIndex::from_scan(source, &backend)
    }

    fn build_index_from_ast(source: &str) -> DocumentIndex {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse(source).unwrap();
        DocumentIndex::from_ast(ast)
    }

    #[test]
    fn test_from_scan_empty_document() {
        let index = build_index_from_scan("");
        assert!(index.headings().is_empty());
        assert!(index.wiki_links().is_empty());
        assert!(index.tags().is_empty());
        assert!(index.markdown_links().is_empty());
        assert!(index.toc().is_empty());
    }

    #[test]
    fn test_from_scan_single_heading() {
        let index = build_index_from_scan("# Hello\n");
        assert_eq!(index.headings().len(), 1);
        assert_eq!(index.headings()[0].text, "Hello");
        assert_eq!(index.headings()[0].level, 1);
        assert_eq!(index.headings()[0].slug, "hello");
    }

    #[test]
    fn test_from_scan_multiple_headings() {
        let index = build_index_from_scan("# First\n\n## Second\n\n### Third\n");
        assert_eq!(index.headings().len(), 3);
        assert_eq!(index.headings()[0].level, 1);
        assert_eq!(index.headings()[1].level, 2);
        assert_eq!(index.headings()[2].level, 3);
        assert!(index.heading_by_slug("first").is_some());
        assert!(index.heading_by_slug("second").is_some());
    }

    #[test]
    fn test_from_scan_toc_builds() {
        let index = build_index_from_scan("# Root\n\n## Child\n\n### Grandchild\n");
        let toc = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 2);
    }

    #[test]
    fn test_from_scan_outline_builds() {
        let index = build_index_from_scan("# Root\n\n## Child\n");
        let outline = index.outline();
        assert_eq!(outline.children.len(), 1);
        assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
    }

    #[test]
    fn test_from_scan_markdown_links() {
        let index = build_index_from_scan("See [example](https://example.com) here\n");
        assert_eq!(index.markdown_links().len(), 1);
        assert_eq!(index.markdown_links()[0].text, "example");
        assert_eq!(index.markdown_links()[0].url, "https://example.com");
    }

    #[test]
    fn test_from_scan_wiki_links() {
        let index = build_index_from_scan("See [[My Page]] here\n");
        assert_eq!(index.wiki_links().len(), 1);
        assert_eq!(index.wiki_links()[0].target, "My Page");
    }

    #[test]
    fn test_from_scan_tags() {
        let index = build_index_from_scan("text #topic #project\n");
        assert!(index.tags().len() >= 2);
        assert!(index.tags().iter().any(|t| t.name == "topic"));
        assert!(index.tags().iter().any(|t| t.name == "project"));
    }

    #[test]
    fn test_from_scan_block_ids() {
        let index = build_index_from_scan("some content ^my-block\n");
        assert!(index.block_by_id("my-block").is_some());
    }

    #[test]
    fn test_from_scan_xml_tags_empty() {
        let index = build_index_from_scan("<goal>Ship</goal>\n");
        assert!(index.xml_tags().is_empty());
    }

    #[test]
    fn test_from_ast_unchanged() {
        let index = build_index_from_ast("# Heading\n\n[[Page]]\n#tag\n");
        assert_eq!(index.headings()[0].text, "Heading");
        assert!(!index.wiki_links().is_empty());
        assert!(index.tags().iter().any(|t| t.name == "tag"));
    }

    #[test]
    fn test_parity_headings() {
        let text = "# First\n\n## Second\n\n### Third\n";
        let ast_idx = build_index_from_ast(text);
        let scan_idx = build_index_from_scan(text);

        assert_eq!(ast_idx.headings().len(), scan_idx.headings().len());
        for (a, s) in ast_idx.headings().iter().zip(scan_idx.headings().iter()) {
            assert_eq!(a.text, s.text);
            assert_eq!(a.level, s.level);
            assert_eq!(a.slug, s.slug);
        }
    }

    // --- Bug fix tests: wiki link range calculation (marky-x3x #1) ---

    #[test]
    fn test_from_scan_wiki_link_range_no_alias() {
        let index = build_index_from_scan("See [[My Page]] here\n");
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "My Page");
        assert_eq!(wl.range.start, Position::new(0, 4));
        assert_eq!(wl.range.end, Position::new(0, 15));
    }

    #[test]
    fn test_from_scan_wiki_link_range_with_alias() {
        let index = build_index_from_scan("See [[target|display]] here\n");
        let wl = &index.wiki_links()[0];
        assert_eq!(wl.target, "target");
        assert!(wl.alias.is_some());
        assert_eq!(wl.alias.unwrap(), "display");
        assert_eq!(wl.range.start, Position::new(0, 4));
        assert_eq!(wl.range.end, Position::new(0, 22));
    }

    #[test]
    fn test_from_scan_markdown_link_range() {
        let index = build_index_from_scan("See [example](https://example.com) here\n");
        let ml = &index.markdown_links()[0];
        assert_eq!(ml.text, "example");
        assert_eq!(ml.range.start, Position::new(0, 4));
        assert_eq!(ml.range.end, Position::new(0, 34));
    }

    // --- Bug fix test: block ID range (marky-x3x #2) ---

    #[test]
    fn test_from_scan_block_id_range_nonzero_width() {
        let index = build_index_from_scan("some content ^my-block\n");
        let block = index.block_by_id("my-block").unwrap();
        assert_eq!(block.range.start, Position::new(0, 13));
        assert_eq!(block.range.end, Position::new(0, 22));
        assert_ne!(block.range.start, block.range.end);
    }
}
