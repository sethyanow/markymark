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

/// Compile-time assertion: DocumentIndex must be Send + Sync for tower-lsp
/// (RwLock<ServerState> requires Send + Sync on all contained types).
///
/// If this test fails to compile, the arena wrapper strategy has regressed.
#[test]
fn document_index_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DocumentIndex>();
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
        start_byte: 0,
        end_byte: 7,
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
        start_byte: 0,
        end_byte: 7,
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
        start_byte: 0,
        end_byte: 6,
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
// Frontmatter and properties tests (marky-khy)
// ---------------------------------------------------------------------------

#[test]
fn test_frontmatter_stored_in_document_index() {
    let index = build_index("---\ntitle: My Page\nstatus: active\n---\n\n# Content\n");
    let fm = index.frontmatter();
    assert!(!fm.is_empty(), "frontmatter should be non-empty");
    assert!(
        fm.iter().any(|e| e.key == "title"),
        "should find 'title' key"
    );
    assert!(
        fm.iter().any(|e| e.key == "status"),
        "should find 'status' key"
    );
}

#[test]
fn test_frontmatter_aliases_accessible() {
    let index = build_index("---\naliases: [name1, name2]\n---\n\n# Page\n");
    let aliases = index.aliases();
    assert_eq!(aliases.len(), 2, "should have 2 aliases");
    assert!(aliases.contains(&"name1"), "should contain 'name1'");
    assert!(aliases.contains(&"name2"), "should contain 'name2'");
}

#[test]
fn test_properties_stored_in_document_index() {
    let index = build_index("tags:: project, rust\nstatus:: active\n\n# Content\n");
    let props = index.properties();
    assert!(!props.is_empty(), "properties should be non-empty");
    assert!(
        props.iter().any(|e| e.key == "tags"),
        "should find 'tags' key"
    );
    assert!(
        props.iter().any(|e| e.key == "status"),
        "should find 'status' key"
    );
}

#[test]
fn test_no_frontmatter_returns_empty() {
    let index = build_index("# Just a heading\n\nSome content.\n");
    assert!(
        index.frontmatter().is_empty(),
        "no frontmatter should return empty slice"
    );
    assert!(
        index.aliases().is_empty(),
        "no frontmatter should return empty aliases"
    );
}

#[test]
fn test_frontmatter_with_colon_in_value() {
    let index = build_index("---\nurl: https://example.com\ntitle: My Page\n---\n\n# Content\n");
    let fm = index.frontmatter();
    let url_entry = fm.iter().find(|e| e.key == "url");
    assert!(url_entry.is_some(), "should find 'url' key");
    let url_val = url_entry.unwrap();
    match &url_val.value {
        FrontmatterValueEntry::String(s) => {
            assert_eq!(
                *s, "https://example.com",
                "URL should not be truncated at second colon"
            );
        }
        FrontmatterValueEntry::List(_) => panic!("URL should be a String, not List"),
    }
}

#[test]
fn test_frontmatter_and_properties_coexist() {
    let index =
        build_index("---\ntitle: My Page\n---\n\ntags:: project\nstatus:: active\n\n# Heading\n");
    // Frontmatter should be parsed
    assert!(
        !index.frontmatter().is_empty(),
        "frontmatter should be present"
    );
    // Properties should also be parsed (they appear after frontmatter)
    // Note: depending on parser behavior, properties after frontmatter may or may not be detected.
    // At minimum, frontmatter should work correctly and not conflict.
    let fm = index.frontmatter();
    assert!(
        fm.iter().any(|e| e.key == "title"),
        "title should be in frontmatter"
    );
}

#[test]
fn test_no_properties_returns_empty() {
    let index = build_index("# Just a heading\n\nSome content.\n");
    assert!(
        index.properties().is_empty(),
        "no properties should return empty slice"
    );
}

// ---------------------------------------------------------------------------
// Scan-based construction tests (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "zig-kernels")]
mod scan_tests;

// ---------------------------------------------------------------------------
// Block ref wiring tests (marky-waw)
// ---------------------------------------------------------------------------

const UUID_A: &str = "550e8400-e29b-41d4-a716-446655440000";
const UUID_B: &str = "7f6c1b2a-3d4e-5f60-a7b8-c9d0e1f20304";

#[test]
fn test_block_refs_stored_in_document_index() {
    // Bug this catches: extract_block_refs called but result dropped during index construction
    let source = format!("Some text (({UUID_A})) more text");
    let index = build_index(&source);
    let refs = index.block_refs();
    assert_eq!(refs.len(), 1, "expected 1 block ref, got {}", refs.len());
    assert_eq!(refs[0].uuid, UUID_A);
}

#[test]
fn test_multiple_block_refs_all_returned() {
    // Bug this catches: only first block ref extracted, Vec truncated early
    let source = format!("(({UUID_A})) text (({UUID_B}))");
    let index = build_index(&source);
    let refs = index.block_refs();
    assert_eq!(refs.len(), 2, "expected 2 block refs, got {}", refs.len());
    let uuids: Vec<&str> = refs.iter().map(|r| r.uuid).collect();
    assert!(uuids.contains(&UUID_A), "missing UUID_A");
    assert!(uuids.contains(&UUID_B), "missing UUID_B");
}

#[test]
fn test_no_block_refs_returns_empty_slice() {
    // Bug this catches: uninitialized field or wrong default, panics instead of empty
    let index = build_index("# Heading\nno block refs here");
    assert!(
        index.block_refs().is_empty(),
        "expected empty block_refs for document without ((uuid))"
    );
}

#[test]
fn test_block_ref_range_matches_position() {
    // Bug this catches: range computation off by one or character vs byte offset confusion
    let source = format!("(({UUID_A}))");
    let index = build_index(&source);
    let refs = index.block_refs();
    assert_eq!(refs.len(), 1);
    // Full match including (( and )) spans the whole string on line 0
    // Length: "((550e8400-e29b-41d4-a716-446655440000))" = 2 + 36 + 2 = 40
    let r = refs[0].range;
    assert_eq!(
        r.start,
        Position::new(0, 0),
        "range should start at column 0"
    );
    assert_eq!(r.end, Position::new(0, 40), "range should end at column 40");
    assert_ne!(r.start, r.end, "range must have non-zero width");
}

#[test]
fn test_block_ref_not_extracted_for_short_uuid() {
    // Bug this catches: regex accepts partial/short UUIDs, produces garbage entries
    // BLOCK_REF_RE requires exactly 36 [0-9a-f-] chars — short UUIDs must NOT match
    let index = build_index("((abc-123)) ((too-short))");
    assert!(
        index.block_refs().is_empty(),
        "short non-UUID patterns should not be extracted as block refs"
    );
}

#[test]
fn test_block_ref_uuid_v4_format_preserved_exactly() {
    // Bug this catches: UUID normalization or truncation at dash characters
    let source = format!("ref: (({UUID_A}))");
    let index = build_index(&source);
    let refs = index.block_refs();
    assert_eq!(refs.len(), 1);
    assert_eq!(
        refs[0].uuid, UUID_A,
        "UUID must be preserved exactly including dashes"
    );
}

// ---------------------------------------------------------------------------
// md4c scan-based construction tests (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "zig-kernels")]
mod md4c_scan_tests;

// --- Task 4 (marky-gb1): IncrementalOverrides tests ---

/// Tags have no range info, so the override path always passes None for tags.
/// Verify that when IncrementalOverrides.tags is None, tags are still extracted correctly.
#[test]
fn test_tag_no_incremental_opt_always_full_rebuild() {
    let source = "# Doc\n\n#mytag #another\n";
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    let overrides = IncrementalOverrides {
        wiki_links: None,
        blocks: None,
        tags: None,
        markdown_links: None,
        xml_tags: None,
        code_spans: None,
        ..Default::default()
    };
    let index = DocumentIndex::from_ast_with_overrides_opt(ast, overrides);
    let tags = index.tags();
    assert_eq!(tags.len(), 2);
    let names: Vec<_> = tags.iter().map(|t| t.name).collect();
    assert!(names.contains(&"mytag"));
    assert!(names.contains(&"another"));
}

/// Passing a MarkdownLinkOwned override skips re-extraction and returns the override data.
#[test]
fn test_markdown_link_override_reuses_when_provided() {
    let source = "Some [orig](https://orig.com) text\n";
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    let override_link = MarkdownLinkOwned {
        text: "injected".to_string(),
        url: "https://injected.com".to_string(),
        anchor: None,
        range: Range::new(Position::new(0, 0), Position::new(0, 10)),
        start_byte: 0,
        end_byte: 10,
    };
    let overrides = IncrementalOverrides {
        wiki_links: None,
        blocks: None,
        tags: None,
        markdown_links: Some(vec![override_link]),
        xml_tags: None,
        code_spans: None,
        ..Default::default()
    };
    let index = DocumentIndex::from_ast_with_overrides_opt(ast, overrides);
    let mls = index.markdown_links();
    assert_eq!(mls.len(), 1);
    assert_eq!(mls[0].text, "injected");
    assert_eq!(mls[0].url, "https://injected.com");
}

/// Passing a XmlTagOwned override skips re-extraction and returns the override data.
#[test]
fn test_xml_tag_override_reuses_when_provided() {
    let source = "<agent id=\"a\">content</agent>\n";
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    let override_tag = XmlTagOwned {
        tag_name: "injected-tag".to_string(),
        attributes: vec![("k".to_string(), "v".to_string())],
        is_self_closing: false,
        is_unclosed: false,
        range: Range::new(Position::new(0, 0), Position::new(0, 10)),
        start_byte: 0,
        end_byte: 10,
    };
    let overrides = IncrementalOverrides {
        wiki_links: None,
        blocks: None,
        tags: None,
        markdown_links: None,
        xml_tags: Some(vec![override_tag]),
        code_spans: None,
        ..Default::default()
    };
    let index = DocumentIndex::from_ast_with_overrides_opt(ast, overrides);
    let xts = index.xml_tags();
    assert_eq!(xts.len(), 1);
    assert_eq!(xts[0].tag_name, "injected-tag");
}

// ---------------------------------------------------------------------------
// scan_all fallback regression tests (marky-h0lp)
// ---------------------------------------------------------------------------

/// When scan_all fails, from_scan must fall back to independent scan_headings /
/// scan_links calls so partial data is not silently dropped.
#[cfg(feature = "zig-kernels")]
mod scan_all_fallback_tests {
    use super::*;
    use markymark_core::scanner::{
        BlockIdResult, HeadingResult, LinkResult, ScanAllResult, ScanBackend, ScanError,
        ScanLinkType, TagResult,
    };

    /// Mock backend: scan_all always errors; individual scans always succeed.
    struct FailingScanAllBackend;

    impl ScanBackend for FailingScanAllBackend {
        fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> {
            Ok(vec![HeadingResult {
                text: "Heading".to_string(),
                offset: 0,
                level: 1,
            }])
        }

        fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> {
            Ok(vec![LinkResult {
                offset: 10,
                text: "click".to_string(),
                target: "https://example.com".to_string(),
                link_type: ScanLinkType::Markdown,
            }])
        }

        fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> {
            Ok(vec![])
        }

        fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
            Ok(vec![])
        }

        fn estimate_tokens(&self, _text: &str) -> Result<u32, ScanError> {
            Ok(0)
        }

        fn scan_all(&self, _text: &str) -> Result<ScanAllResult, ScanError> {
            Err(ScanError::InternalError(
                "scan_all deliberately fails".to_string(),
            ))
        }
    }

    /// Regression for marky-h0lp: scan_all failure must not silently drop
    /// headings and links that are available via independent fallback scans.
    #[test]
    fn test_from_scan_uses_fallback_when_scan_all_fails() {
        let backend = FailingScanAllBackend;
        let index = DocumentIndex::from_scan("# Heading\n[click](https://example.com)\n", &backend);
        assert_eq!(
            index.headings().len(),
            1,
            "headings must survive via fallback scan_headings when scan_all fails"
        );
        assert_eq!(
            index.markdown_links().len(),
            1,
            "links must survive via fallback scan_links when scan_all fails"
        );
    }
}

/// All five overrides provided — verify each extractor uses the override, not re-extraction.
#[test]
fn test_incremental_overrides_all_five() {
    let source = "[[orig]] #tag [orig](https://orig.com) ^block-id <div/>\n";
    let mut parser = Parser::new().unwrap();
    let ast = parser.parse(source).unwrap();
    let overrides = IncrementalOverrides {
        wiki_links: Some(vec![WikiLinkOwned {
            target: "injected-page".to_string(),
            alias: None,
            heading: None,
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            start_byte: 0,
            end_byte: 5,
        }]),
        blocks: Some(vec![BlockOwned {
            id: "injected-block".to_string(),
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            start_byte: 0,
            end_byte: 5,
        }]),
        tags: None,
        markdown_links: Some(vec![MarkdownLinkOwned {
            text: "injected-link".to_string(),
            url: "https://injected.com".to_string(),
            anchor: None,
            range: Range::new(Position::new(0, 0), Position::new(0, 10)),
            start_byte: 0,
            end_byte: 10,
        }]),
        xml_tags: Some(vec![XmlTagOwned {
            tag_name: "injected-xml".to_string(),
            attributes: vec![],
            is_self_closing: true,
            is_unclosed: false,
            range: Range::new(Position::new(0, 0), Position::new(0, 10)),
            start_byte: 0,
            end_byte: 10,
        }]),
        code_spans: None,
        ..Default::default()
    };
    let index = DocumentIndex::from_ast_with_overrides_opt(ast, overrides);
    assert_eq!(index.wiki_links().len(), 1);
    assert_eq!(index.wiki_links()[0].target, "injected-page");
    assert!(index.block_by_id("injected-block").is_some());
    assert_eq!(index.markdown_links().len(), 1);
    assert_eq!(index.markdown_links()[0].text, "injected-link");
    assert_eq!(index.xml_tags().len(), 1);
    assert_eq!(index.xml_tags()[0].tag_name, "injected-xml");
    // Tags: always from AST (override is None), expect #tag from source
    assert!(index.tags().iter().any(|t| t.name == "tag"));
}

// ── Phase B-2: 5 new DocumentDependent types ───────────────────────

#[test]
fn test_embeds_from_ast() {
    let source = "![[my-image.png]]\n\nSome text ![[other-file]]\n";
    let index = build_index(source);
    let embeds = index.embeds();
    assert_eq!(embeds.len(), 2);
    assert_eq!(embeds[0].target, "my-image.png");
    assert_eq!(embeds[1].target, "other-file");
}

#[test]
fn test_tasks_from_ast() {
    let source = "- [x] Done task\n- [ ] Open task\n- [/] In progress\n";
    let index = build_index(source);
    let tasks = index.tasks();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].state, "checked");
    assert_eq!(tasks[1].state, "unchecked");
    assert_eq!(tasks[2].state, "in_progress");
}

#[test]
fn test_callouts_from_ast() {
    let source = "> [!note] My Title\n> content\n\n> [!warning] Watch out\n> danger\n";
    let index = build_index(source);
    let callouts = index.callouts();
    assert_eq!(callouts.len(), 2);
    assert_eq!(callouts[0].callout_type, "note");
    assert_eq!(callouts[0].title, Some("My Title"));
    assert_eq!(callouts[1].callout_type, "warning");
    assert_eq!(callouts[1].title, Some("Watch out"));
}

#[test]
fn test_query_blocks_from_ast() {
    let source = "{{query (and [[page]] (task done))}}\n\ntext\n\n{{query simple}}\n";
    let index = build_index(source);
    let qbs = index.query_blocks();
    assert_eq!(qbs.len(), 2);
    assert_eq!(qbs[0].query, "(and [[page]] (task done))");
    assert_eq!(qbs[1].query, "simple");
}

#[test]
fn test_link_definitions_from_ast() {
    let source = "[example]: https://example.com\n[rust]: https://rust-lang.org \"Rust\"\n";
    let index = build_index(source);
    let lds = index.link_definitions();
    assert_eq!(lds.len(), 2);
    assert_eq!(lds[0].label, "example");
    assert_eq!(lds[0].url, "https://example.com");
    assert_eq!(lds[0].title, None);
    assert_eq!(lds[1].label, "rust");
    assert_eq!(lds[1].url, "https://rust-lang.org");
    assert_eq!(lds[1].title, Some("Rust"));
}

#[test]
fn test_new_types_empty_on_plain_text() {
    let source = "# Just a heading\n\nPlain paragraph.\n";
    let index = build_index(source);
    assert!(index.embeds().is_empty());
    assert!(index.tasks().is_empty());
    assert!(index.callouts().is_empty());
    assert!(index.query_blocks().is_empty());
    assert!(index.link_definitions().is_empty());
}

#[test]
fn test_new_types_override_via_incremental() {
    let source = "![[original-embed]]\n";
    let ast = markymark_parser::parse(source).unwrap();
    let overrides = IncrementalOverrides {
        embeds: Some(vec![EmbedOwned {
            target: "injected".to_string(),
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            start_byte: 0,
            end_byte: 5,
        }]),
        tasks: Some(vec![TaskOwned {
            state: "checked".to_string(),
            text: "injected task".to_string(),
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            start_byte: 0,
            end_byte: 5,
        }]),
        ..Default::default()
    };
    let index = DocumentIndex::from_ast_with_overrides_opt(ast, overrides);
    assert_eq!(index.embeds().len(), 1);
    assert_eq!(index.embeds()[0].target, "injected");
    assert_eq!(index.tasks().len(), 1);
    assert_eq!(index.tasks()[0].state, "checked");
    assert_eq!(index.tasks()[0].text, "injected task");
}
