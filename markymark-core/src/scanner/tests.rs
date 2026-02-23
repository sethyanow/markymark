//! Tests for scan backend types and implementations.

use super::*;

// Dummy implementation for compile-time trait checks.
struct DummyScanBackend;

impl ScanBackend for DummyScanBackend {
    fn scan_headings(&self, _text: &str) -> Result<Vec<HeadingResult>, ScanError> {
        Ok(Vec::new())
    }

    fn scan_links(&self, _text: &str) -> Result<Vec<LinkResult>, ScanError> {
        Ok(Vec::new())
    }

    fn scan_tags(&self, _text: &str) -> Result<Vec<TagResult>, ScanError> {
        Ok(Vec::new())
    }

    fn scan_block_ids(&self, _text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
        Ok(Vec::new())
    }

    fn estimate_tokens(&self, _text: &str) -> Result<u32, ScanError> {
        Ok(0)
    }
}

#[test]
fn test_scan_backend_trait_object() {
    // Verifies ScanBackend is object-safe (dyn-compatible).
    let backend: Box<dyn ScanBackend> = Box::new(DummyScanBackend);
    let result = backend.scan_headings("# Hello");
    assert!(result.is_ok());
}

#[test]
fn test_scan_backend_send_sync() {
    // Verifies ScanBackend implementations are Send + Sync.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DummyScanBackend>();

    // Also verify the trait object is Send + Sync.
    fn assert_dyn_send_sync(_: &(dyn ScanBackend + Send + Sync)) {}
    let backend = DummyScanBackend;
    assert_dyn_send_sync(&backend);
}

#[test]
fn test_scan_error_display() {
    let err = ScanError::InvalidInput("bad text".to_string());
    assert_eq!(err.to_string(), "scan: invalid input: bad text");

    let err = ScanError::InternalError("crash".to_string());
    assert_eq!(err.to_string(), "scan: internal error: crash");
}

#[test]
fn test_heading_result_fields() {
    let h = HeadingResult {
        text: "Hello".to_string(),
        offset: 2,
        level: 1,
    };
    assert_eq!(h.text, "Hello");
    assert_eq!(h.offset, 2);
    assert_eq!(h.level, 1);
}

#[test]
fn test_link_result_fields() {
    let l = LinkResult {
        offset: 0,
        text: "click".to_string(),
        target: "https://example.com".to_string(),
        link_type: ScanLinkType::Markdown,
    };
    assert_eq!(l.link_type, ScanLinkType::Markdown);

    let w = LinkResult {
        offset: 0,
        text: "Page".to_string(),
        target: "My Page".to_string(),
        link_type: ScanLinkType::Wiki,
    };
    assert_eq!(w.link_type, ScanLinkType::Wiki);
}

#[test]
fn test_tag_result_fields() {
    let t = TagResult {
        name: "topic".to_string(),
        offset: 5,
    };
    assert_eq!(t.name, "topic");
}

#[test]
fn test_block_id_result_fields() {
    let b = BlockIdResult {
        id: "my-block".to_string(),
        offset: 10,
    };
    assert_eq!(b.id, "my-block");
}

// --- scan_all default impl tests (no feature gate) ---

#[test]
fn test_scan_all_headings_match_scan_headings() {
    let backend = DummyScanBackend;
    let all = backend.scan_all("# Hello\n").unwrap();
    let headings = backend.scan_headings("# Hello\n").unwrap();
    assert_eq!(all.headings, headings);
}

#[test]
fn test_scan_all_links_match_scan_links() {
    let backend = DummyScanBackend;
    let all = backend.scan_all("[click](https://example.com)\n").unwrap();
    let links = backend
        .scan_links("[click](https://example.com)\n")
        .unwrap();
    assert_eq!(all.links, links);
}

#[test]
fn test_scan_all_empty_text_returns_default() {
    let backend = DummyScanBackend;
    let all = backend.scan_all("").unwrap();
    assert!(all.headings.is_empty());
    assert!(all.links.is_empty());
}

// --- Md4cScanBackend tests (feature-gated) ---

#[cfg(feature = "zig-kernels")]
mod md4c_tests {
    use super::super::*;

    #[test]
    fn test_md4c_scan_backend_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Md4cScanBackend>();
    }

    #[test]
    fn test_md4c_scan_backend_trait_object() {
        let backend: Box<dyn ScanBackend> = Box::new(Md4cScanBackend);
        let result = backend.scan_headings("# Hello");
        assert!(result.is_ok());
    }

    #[test]
    fn test_md4c_scan_headings_basic() {
        let backend = Md4cScanBackend;
        let results = backend.scan_headings("# Hello\n").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello");
        assert_eq!(results[0].level, 1);
        assert_eq!(results[0].offset, 0);
    }

    #[test]
    fn test_md4c_scan_links_markdown() {
        let backend = Md4cScanBackend;
        let results = backend
            .scan_links("[click](https://example.com)\n")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "click");
        assert_eq!(results[0].target, "https://example.com");
        assert_eq!(results[0].link_type, ScanLinkType::Markdown);
    }

    #[test]
    fn test_md4c_scan_links_wiki() {
        let backend = Md4cScanBackend;
        let results = backend.scan_links("[[Target]]\n").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].link_type, ScanLinkType::Wiki);
        assert_eq!(results[0].target, "Target");
    }

    #[test]
    fn test_md4c_scan_empty_input() {
        let backend = Md4cScanBackend;
        assert!(backend.scan_headings("").unwrap().is_empty());
        assert!(backend.scan_links("").unwrap().is_empty());
        assert!(backend.scan_tags("").unwrap().is_empty());
        assert!(backend.scan_block_ids("").unwrap().is_empty());
        assert_eq!(backend.estimate_tokens("").unwrap(), 0);
    }

    #[test]
    fn test_md4c_scan_entity_decoded() {
        // md4c ExtractionRenderer decodes HTML entities to UTF-8 (marky-yfh7)
        let backend = Md4cScanBackend;
        let results = backend.scan_headings("# Hello &amp; World\n").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello & World");
    }

    #[test]
    fn test_scan_all_combined_doc() {
        // scan_all returns both headings and links from a document containing both.
        let backend = Md4cScanBackend;
        let text = "# Heading One\n\n[link](https://example.com)\n";
        let all = backend.scan_all(text).unwrap();
        assert!(!all.headings.is_empty(), "expected headings");
        assert!(!all.links.is_empty(), "expected links");
    }

    #[test]
    fn test_md4c_scan_all_consistent_with_separate() {
        // Md4cScanBackend::scan_all must return identical results to calling
        // scan_headings and scan_links separately.
        let backend = Md4cScanBackend;
        let text = concat!(
            "# Alpha\n",
            "## Beta\n",
            "### Gamma\n",
            "[one](https://one.example.com)\n",
            "[two](https://two.example.com)\n",
            "[[WikiPage]]\n",
        );
        let all = backend.scan_all(text).unwrap();
        let headings = backend.scan_headings(text).unwrap();
        let links = backend.scan_links(text).unwrap();
        assert_eq!(all.headings, headings);
        assert_eq!(all.links, links);
    }

    // ── Code span tests (marky-vsh2) ────────────────────────────────

    #[test]
    fn test_md4c_scan_code_spans() {
        let backend = Md4cScanBackend;
        let result = backend.scan_code_spans("Hello `world` end").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "world");
        assert_eq!(result[0].offset, 6);
        assert_eq!(result[0].end_offset, 13);
    }

    #[test]
    fn test_scan_all_includes_code_spans() {
        let backend = Md4cScanBackend;
        let text = "# Heading\n\n`code` and `more`\n";
        let all = backend.scan_all(text).unwrap();
        assert_eq!(all.code_spans.len(), 2);
        assert_eq!(all.code_spans[0].text, "code");
        assert_eq!(all.code_spans[1].text, "more");

        // scan_all code_spans must match scan_code_spans
        let separate = backend.scan_code_spans(text).unwrap();
        assert_eq!(all.code_spans.len(), separate.len());
        for (a, s) in all.code_spans.iter().zip(separate.iter()) {
            assert_eq!(a.text, s.text);
            assert_eq!(a.offset, s.offset);
            assert_eq!(a.end_offset, s.end_offset);
        }
    }

    #[test]
    fn test_md4c_scan_code_spans_empty() {
        let backend = Md4cScanBackend;
        let result = backend.scan_code_spans("No code here").unwrap();
        assert!(result.is_empty());
    }

    // ── Task/Embed tests (marky-bmu9) ───────────────────────────

    #[test]
    fn test_md4c_scan_tasks_unchecked() {
        let backend = Md4cScanBackend;
        let result = backend.scan_tasks("- [ ] Todo\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].state, "unchecked");
        assert_eq!(result[0].text, "Todo");
    }

    #[test]
    fn test_md4c_scan_tasks_checked() {
        let backend = Md4cScanBackend;
        let result = backend.scan_tasks("- [x] Done\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].state, "checked");
    }

    #[test]
    fn test_md4c_scan_embeds() {
        let backend = Md4cScanBackend;
        let result = backend.scan_embeds("![[target]]\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].target, "target");
    }

    #[test]
    fn test_md4c_scan_no_embed_for_wikilink() {
        let backend = Md4cScanBackend;
        let result = backend.scan_embeds("[[link]]\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_all_includes_tasks_embeds() {
        let backend = Md4cScanBackend;
        let text = "- [x] Done\n\n![[embed]]\n";
        let all = backend.scan_all(text).unwrap();
        assert_eq!(all.tasks.len(), 1);
        assert_eq!(all.tasks[0].state, "checked");
        assert_eq!(all.embeds.len(), 1);
        assert_eq!(all.embeds[0].target, "embed");
    }

    // --- Callout + Block ref tests (marky-8ac8) ---

    #[test]
    fn test_md4c_scan_callouts_basic() {
        let backend = Md4cScanBackend;
        let result = backend.scan_callouts("> [!note]\n> Content\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].callout_type, "note");
        assert!(result[0].title.is_none());
    }

    #[test]
    fn test_md4c_scan_callouts_with_title() {
        let backend = Md4cScanBackend;
        let result = backend
            .scan_callouts("> [!tip] My Title\n> Content\n")
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].callout_type, "tip");
        assert_eq!(result[0].title.as_deref(), Some("My Title"));
    }

    #[test]
    fn test_md4c_scan_block_refs() {
        let backend = Md4cScanBackend;
        let result = backend
            .scan_block_refs("Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n")
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uuid, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn test_md4c_scan_block_refs_invalid_rejected() {
        let backend = Md4cScanBackend;
        let result = backend.scan_block_refs("((not-valid))\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_all_includes_callouts_block_refs() {
        let backend = Md4cScanBackend;
        let text =
            "> [!warning]\n> Watch out\n\nText ((a1b2c3d4-e5f6-7890-abcd-ef1234567890))\n";
        let all = backend.scan_all(text).unwrap();
        assert_eq!(all.callouts.len(), 1);
        assert_eq!(all.callouts[0].callout_type, "warning");
        assert_eq!(all.block_refs.len(), 1);
        assert_eq!(
            all.block_refs[0].uuid,
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        );
    }

    // --- Query block + Link definition tests (B-5) ---

    #[test]
    fn test_md4c_scan_link_definitions() {
        let backend = Md4cScanBackend;
        let result = backend
            .scan_link_definitions("[label]: https://example.com\n")
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "label");
        assert_eq!(result[0].url, "https://example.com");
        assert!(result[0].title.is_none());
    }

    #[test]
    fn test_md4c_scan_link_definitions_with_title() {
        let backend = Md4cScanBackend;
        let result = backend
            .scan_link_definitions("[label]: https://example.com \"My Title\"\n")
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "label");
        assert_eq!(result[0].url, "https://example.com");
        assert_eq!(result[0].title.as_deref(), Some("My Title"));
    }

    #[test]
    fn test_md4c_scan_link_definitions_empty() {
        let backend = Md4cScanBackend;
        let result = backend.scan_link_definitions("No link defs\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_all_includes_link_definitions() {
        let backend = Md4cScanBackend;
        let text = "# Heading\n\n[label]: https://example.com\n";
        let all = backend.scan_all(text).unwrap();
        assert_eq!(all.link_definitions.len(), 1);
        assert_eq!(all.link_definitions[0].label, "label");
    }
}

#[test]
fn test_default_scan_code_spans_empty() {
    // The default trait implementation returns an empty vec.
    let backend = DummyScanBackend;
    let result = backend.scan_code_spans("Hello `world` end").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_tasks_empty() {
    let backend = DummyScanBackend;
    let result = backend.scan_tasks("- [x] Task").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_embeds_empty() {
    let backend = DummyScanBackend;
    let result = backend.scan_embeds("![[embed]]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_callouts_empty() {
    let backend = DummyScanBackend;
    let result = backend.scan_callouts("> [!note]\n> Content").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_block_refs_empty() {
    let backend = DummyScanBackend;
    let result = backend
        .scan_block_refs("((a1b2c3d4-e5f6-7890-abcd-ef1234567890))")
        .unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_query_blocks_empty() {
    let backend = DummyScanBackend;
    let result = backend.scan_query_blocks("{{query items}}").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_default_scan_link_definitions_empty() {
    let backend = DummyScanBackend;
    let result = backend
        .scan_link_definitions("[label]: https://example.com")
        .unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_scan_all_includes_query_blocks_and_link_defs() {
    // Default scan_all returns empty vecs from default trait impls.
    let backend = DummyScanBackend;
    let all = backend.scan_all("").unwrap();
    assert!(all.query_blocks.is_empty());
    assert!(all.link_definitions.is_empty());
}
