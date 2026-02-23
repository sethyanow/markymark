use super::*;

// ---------------------------------------------------------------------------
// scan_all fallback regression tests (marky-h0lp)
// ---------------------------------------------------------------------------

/// When scan_all fails, from_scan must fall back to independent scan_headings /
/// scan_links calls so partial data is not silently dropped.
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
    // md4c only recognizes [x] and [ ] as task markers (CommonMark spec).
    // Logseq's [/] is not recognized by md4c, so only 2 tasks are extracted.
    let source = "- [x] Done task\n- [ ] Open task\n- [/] In progress\n";
    let index = build_index(source);
    let tasks = index.tasks();
    assert_eq!(tasks.len(), 2);
}

#[test]
fn test_callouts_from_ast() {
    let source = "> [!note] My Title\n> content\n\n> [!warning] Watch out\n> danger\n";
    let index = build_index(source);
    let callouts = index.callouts();
    assert_eq!(callouts.len(), 2);
    assert_eq!(callouts[0].callout_type, "note");
    assert_eq!(callouts[1].callout_type, "warning");
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
