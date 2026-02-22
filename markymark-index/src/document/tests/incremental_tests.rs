use super::*;

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
