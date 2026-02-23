use super::*;

#[test]
fn test_extract_heading() {
    let result = extract_md4c("# Hello\n").unwrap();
    assert_eq!(result.headings.len(), 1);
    assert_eq!(result.headings[0].text, "Hello");
    assert_eq!(result.headings[0].level, 1);
    assert_eq!(result.headings[0].source_offset, 0);
}

#[test]
fn test_extract_link() {
    let result = extract_md4c("[click](https://example.com)\n").unwrap();
    assert_eq!(result.links.len(), 1);
    assert_eq!(result.links[0].text, "click");
    assert_eq!(result.links[0].target, "https://example.com");
    assert!(!result.links[0].is_wiki);
}

#[test]
fn test_extract_wiki_link() {
    let result = extract_md4c("[[Target]]\n").unwrap();
    assert_eq!(result.links.len(), 1);
    assert!(result.links[0].is_wiki);
    assert_eq!(result.links[0].target, "Target");
}

#[test]
fn test_extract_mixed_document() {
    let input =
        "# Title\n\nSome [link](url) text.\n\n## Section\n\nSee [[Wiki]] for details.\n";
    let result = extract_md4c(input).unwrap();
    assert_eq!(result.headings.len(), 2);
    assert_eq!(result.headings[0].text, "Title");
    assert_eq!(result.headings[1].text, "Section");
    assert_eq!(result.links.len(), 2);
    assert!(!result.links[0].is_wiki);
    assert!(result.links[1].is_wiki);
}

#[test]
fn test_extract_empty_input() {
    let result = extract_md4c("").unwrap();
    assert!(result.headings.is_empty());
    assert!(result.links.is_empty());
}

#[test]
fn test_extract_entity_decoded() {
    // ExtractionRenderer decodes HTML entities to UTF-8 (marky-yfh7)
    let result = extract_md4c("# Hello &amp; World\n").unwrap();
    assert_eq!(result.headings[0].text, "Hello & World");
}

#[test]
fn test_text_is_valid_utf8() {
    let result = extract_md4c("# Héllo Wörld\n").unwrap();
    assert_eq!(result.headings[0].text, "Héllo Wörld");
}

#[test]
fn test_struct_sizes_match_zig() {
    assert_eq!(std::mem::size_of::<CMd4cHeading>(), 16);
    assert_eq!(std::mem::size_of::<CMd4cLink>(), 24);
    assert_eq!(std::mem::size_of::<CMd4cCodeSpan>(), 16);
    assert_eq!(std::mem::size_of::<CMd4cTask>(), 20);
    assert_eq!(std::mem::size_of::<CMd4cEmbed>(), 16);
    assert_eq!(std::mem::size_of::<CMd4cCallout>(), 24);
    assert_eq!(std::mem::size_of::<CMd4cBlockRef>(), 12);
    assert_eq!(std::mem::size_of::<CMd4cQueryBlock>(), 16);
    assert_eq!(std::mem::size_of::<CMd4cLinkDefinition>(), 32);
    assert_eq!(std::mem::size_of::<CMd4cProperty>(), 20);
    assert_eq!(std::mem::size_of::<CMd4cXmlTag>(), 28);
    // 12 pointers (96) + 12 u32 counts (48) = 144
    assert_eq!(std::mem::size_of::<CMd4cResult>(), 144);
}

/// Regression test for T2-11: silent `.unwrap_or("")` masked data corruption.
///
/// If the blob contains invalid UTF-8 (e.g. due to Zig packing bugs), the
/// old code silently returned an empty string. The fixed code returns
/// `KernelError::InternalError(-100)` so callers can detect corruption.
#[test]
fn test_invalid_utf8_in_heading_blob_returns_error() {
    // 0xFF 0xFE is invalid UTF-8.
    let blob = [0xFF_u8, 0xFE, b'!'];
    let heading = CMd4cHeading {
        source_offset: 0,
        text_offset: 0,
        text_length: 3,
        level: 1,
        _padding: [0, 0, 0],
    };
    let out = CMd4cResult {
        headings: &heading as *const _ as *mut _,
        links: std::ptr::null_mut(),
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: std::ptr::null_mut(),
        text_blob: blob.as_ptr(),
        headings_count: 1,
        links_count: 0,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 0,
        text_blob_len: 3,
    };
    // Before fix: returns Ok(headings[0].text == "") — silent data loss.
    // After fix: returns Err(KernelError::InternalError(-100)).
    let result = convert_result(&out);
    assert!(
        result.is_err(),
        "invalid UTF-8 in blob must return Err, not silently produce empty string"
    );
}

/// Regression test for T2-11: invalid UTF-8 in link target blob.
#[test]
fn test_invalid_utf8_in_link_blob_returns_error() {
    let blob = [b'o', b'k', 0xFF_u8]; // "ok" text, invalid target
    let link = CMd4cLink {
        source_offset: 0,
        text_offset: 0,
        target_offset: 2,
        text_length: 2,
        target_length: 1,
        is_wiki: 0,
        _padding: [0, 0, 0],
    };
    let out = CMd4cResult {
        headings: std::ptr::null_mut(),
        links: &link as *const _ as *mut _,
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: std::ptr::null_mut(),
        text_blob: blob.as_ptr(),
        headings_count: 0,
        links_count: 1,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 0,
        text_blob_len: 3,
    };
    let result = convert_result(&out);
    assert!(
        result.is_err(),
        "invalid UTF-8 in link target blob must return Err"
    );
}

/// Regression test for marky-ta07: heading text_offset beyond blob end must
/// return KernelError, not panic with OOB slice.
#[test]
fn test_oob_heading_offset_returns_error() {
    let blob = [b'h', b'i']; // 2-byte blob
    let heading = CMd4cHeading {
        source_offset: 0,
        text_offset: 5, // past end of 2-byte blob
        text_length: 3,
        level: 1,
        _padding: [0, 0, 0],
    };
    let out = CMd4cResult {
        headings: &heading as *const _ as *mut _,
        links: std::ptr::null_mut(),
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: std::ptr::null_mut(),
        text_blob: blob.as_ptr(),
        headings_count: 1,
        links_count: 0,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 0,
        text_blob_len: blob.len() as u32,
    };
    let result = convert_result(&out);
    assert!(
        matches!(result, Err(KernelError::InternalError(-101))),
        "OOB heading offset must return InternalError(-101), got: {result:?}"
    );
}

/// Regression test for marky-ta07: link target_offset beyond blob end must
/// return KernelError, not panic with OOB slice.
#[test]
fn test_oob_link_offset_returns_error() {
    let blob = [b'o', b'k']; // 2-byte blob
    let link = CMd4cLink {
        source_offset: 0,
        text_offset: 0,
        target_offset: 10, // past end of 2-byte blob
        text_length: 2,
        target_length: 3,
        is_wiki: 0,
        _padding: [0, 0, 0],
    };
    let out = CMd4cResult {
        headings: std::ptr::null_mut(),
        links: &link as *const _ as *mut _,
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: std::ptr::null_mut(),
        text_blob: blob.as_ptr(),
        headings_count: 0,
        links_count: 1,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 0,
        text_blob_len: blob.len() as u32,
    };
    let result = convert_result(&out);
    assert!(
        matches!(result, Err(KernelError::InternalError(-101))),
        "OOB link target offset must return InternalError(-101), got: {result:?}"
    );
}

/// Regression test for marky-ta07: offset + length that overflows usize must
/// return KernelError, not panic or wrap.
#[test]
fn test_overflow_offset_returns_error() {
    let blob = [b'x'; 4];
    let heading = CMd4cHeading {
        source_offset: 0,
        text_offset: u32::MAX, // usize::MAX addition would overflow
        text_length: 1,
        level: 1,
        _padding: [0, 0, 0],
    };
    let out = CMd4cResult {
        headings: &heading as *const _ as *mut _,
        links: std::ptr::null_mut(),
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: std::ptr::null_mut(),
        text_blob: blob.as_ptr(),
        headings_count: 1,
        links_count: 0,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 0,
        text_blob_len: blob.len() as u32,
    };
    let result = convert_result(&out);
    assert!(
        matches!(result, Err(KernelError::InternalError(-101))),
        "overflow offset must return InternalError(-101), got: {result:?}"
    );
}

// --- Code span tests (marky-pdyo) ---

#[test]
fn test_extract_code_span() {
    let result = extract_md4c("here is `hello` world\n").unwrap();
    assert_eq!(result.code_spans.len(), 1);
    assert_eq!(result.code_spans[0].text, "hello");
    assert_eq!(result.code_spans[0].source_offset, 8);
    assert_eq!(result.code_spans[0].end_offset, 15);
}

#[test]
fn test_extract_code_span_mixed_document() {
    let result = extract_md4c("# Title `code` [link](url)\n").unwrap();
    assert_eq!(result.headings.len(), 1);
    assert_eq!(result.links.len(), 1);
    assert_eq!(result.code_spans.len(), 1);
    assert_eq!(result.code_spans[0].text, "code");
}

#[test]
fn test_extract_no_code_spans() {
    let result = extract_md4c("Just plain text.\n").unwrap();
    assert!(result.code_spans.is_empty());
}

#[test]
fn test_extract_multiple_code_spans() {
    let result = extract_md4c("`a` then `b`\n").unwrap();
    assert_eq!(result.code_spans.len(), 2);
    assert_eq!(result.code_spans[0].text, "a");
    assert_eq!(result.code_spans[1].text, "b");
    assert!(result.code_spans[1].source_offset > result.code_spans[0].source_offset);
}

// --- Task/Embed tests (marky-bmu9) ---

#[test]
fn test_extract_task_unchecked() {
    let result = extract_md4c("- [ ] Todo\n").unwrap();
    assert_eq!(result.tasks.len(), 1);
    assert_eq!(result.tasks[0].state, "unchecked");
    assert_eq!(result.tasks[0].text, "Todo");
}

#[test]
fn test_extract_task_checked() {
    let result = extract_md4c("- [x] Done\n").unwrap();
    assert_eq!(result.tasks.len(), 1);
    assert_eq!(result.tasks[0].state, "checked");
    assert_eq!(result.tasks[0].text, "Done");
}

#[test]
fn test_extract_embed() {
    let result = extract_md4c("![[target]]\n").unwrap();
    assert_eq!(result.embeds.len(), 1);
    assert_eq!(result.embeds[0].target, "target");
    // Also a wiki link
    assert_eq!(result.links.len(), 1);
}

#[test]
fn test_extract_no_embed_for_wikilink() {
    let result = extract_md4c("[[link]]\n").unwrap();
    assert!(result.embeds.is_empty());
    assert_eq!(result.links.len(), 1);
}

#[test]
fn test_extract_empty_has_no_tasks_or_embeds() {
    let result = extract_md4c("").unwrap();
    assert!(result.tasks.is_empty());
    assert!(result.embeds.is_empty());
}

#[test]
fn test_extract_plain_text_no_tasks_or_embeds() {
    let result = extract_md4c("Just plain text.\n").unwrap();
    assert!(result.tasks.is_empty());
    assert!(result.embeds.is_empty());
}

// --- Callout tests (marky-8ac8) ---

#[test]
fn test_extract_callout_basic() {
    let result = extract_md4c("> [!note]\n> Some content\n").unwrap();
    assert_eq!(result.callouts.len(), 1);
    assert_eq!(result.callouts[0].callout_type, "note");
    assert!(result.callouts[0].title.is_none());
}

#[test]
fn test_extract_callout_with_title() {
    let result = extract_md4c("> [!tip] My Title\n> Content\n").unwrap();
    assert_eq!(result.callouts.len(), 1);
    assert_eq!(result.callouts[0].callout_type, "tip");
    assert_eq!(result.callouts[0].title.as_deref(), Some("My Title"));
}

#[test]
fn test_extract_no_callout_for_plain_quote() {
    let result = extract_md4c("> Just a regular quote\n").unwrap();
    assert!(result.callouts.is_empty());
}

#[test]
fn test_extract_empty_has_no_callouts_or_block_refs() {
    let result = extract_md4c("").unwrap();
    assert!(result.callouts.is_empty());
    assert!(result.block_refs.is_empty());
}

// --- Block ref tests (marky-8ac8) ---

#[test]
fn test_extract_block_ref_basic() {
    let result =
        extract_md4c("Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n").unwrap();
    assert_eq!(result.block_refs.len(), 1);
    assert_eq!(
        result.block_refs[0].uuid,
        "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    );
}

#[test]
fn test_extract_block_ref_invalid_uuid_rejected() {
    let result = extract_md4c("Text ((not-valid-uuid)) more\n").unwrap();
    assert!(result.block_refs.is_empty());
}

#[test]
fn test_extract_plain_text_no_callouts_or_block_refs() {
    let result = extract_md4c("Just plain text.\n").unwrap();
    assert!(result.callouts.is_empty());
    assert!(result.block_refs.is_empty());
}

// --- Query block + Link definition tests (B-5) ---

#[test]
fn test_extract_empty_has_no_query_blocks_or_link_defs() {
    let result = extract_md4c("").unwrap();
    assert!(result.query_blocks.is_empty());
    assert!(result.link_definitions.is_empty());
}

#[test]
fn test_extract_plain_text_no_query_blocks_or_link_defs() {
    let result = extract_md4c("Just plain text.\n").unwrap();
    assert!(result.query_blocks.is_empty());
    assert!(result.link_definitions.is_empty());
}

#[test]
fn test_extract_link_definition_basic() {
    let result = extract_md4c("[label]: https://example.com\n").unwrap();
    assert_eq!(result.link_definitions.len(), 1);
    assert_eq!(result.link_definitions[0].label, "label");
    assert_eq!(result.link_definitions[0].url, "https://example.com");
    assert!(result.link_definitions[0].title.is_none());
}

#[test]
fn test_extract_link_definition_with_title() {
    let result = extract_md4c("[label]: https://example.com \"My Title\"\n").unwrap();
    assert_eq!(result.link_definitions.len(), 1);
    assert_eq!(result.link_definitions[0].label, "label");
    assert_eq!(result.link_definitions[0].url, "https://example.com");
    assert_eq!(
        result.link_definitions[0].title.as_deref(),
        Some("My Title")
    );
}

#[test]
fn test_extract_xml_tag_block_level() {
    // Block-level HTML: tag on its own line with blank lines around it.
    let input = "\n<custom-tag>\n\nSome content\n\n</custom-tag>\n";
    let result = extract_md4c(input).unwrap();
    assert!(
        !result.xml_tags.is_empty(),
        "should extract at least one XML tag from block-level HTML"
    );
    assert_eq!(result.xml_tags[0].tag_name, "custom-tag");
    assert!(!result.xml_tags[0].is_self_closing);
}

#[test]
fn test_extract_xml_tag_self_closing() {
    let input = "\n<br />\n";
    let result = extract_md4c(input).unwrap();
    assert!(
        !result.xml_tags.is_empty(),
        "should extract self-closing XML tag"
    );
    assert_eq!(result.xml_tags[0].tag_name, "br");
    assert!(result.xml_tags[0].is_self_closing);
}

#[test]
fn test_extract_xml_tags_empty_document() {
    let result = extract_md4c("# Just a heading\n").unwrap();
    assert!(result.xml_tags.is_empty(), "no XML tags in plain markdown");
}

#[test]
fn test_extract_xml_tags_multiple_block_level() {
    // Tags must be on their own lines with blank lines to be block-level HTML.
    // Inline HTML (<tag>content</tag> on one line) is NOT extracted — md4c's
    // inline HTML content may point to internal buffers, not source text.
    let input = "# Heading\n\n<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n";
    let result = extract_md4c(input).unwrap();
    let names: Vec<&str> = result.xml_tags.iter().map(|t| t.tag_name.as_str()).collect();
    assert!(
        names.contains(&"agent"),
        "should find 'agent'; got: {:?}",
        names
    );
    assert!(
        names.contains(&"goal"),
        "should find 'goal'; got: {:?}",
        names
    );
}

#[test]
fn test_extract_xml_tag_convert_result_roundtrip() {
    // Construct a CMd4cResult with xml_tags to test convert_result path.
    let blob = b"divraw html here";
    let xml_tag = CMd4cXmlTag {
        source_offset: 0,
        end_offset: 20,
        tag_name_offset: 0,
        tag_name_length: 3,
        raw_html_offset: 3,
        raw_html_length: 13,
        is_self_closing: 0,
        is_unclosed: 1,
        _pad: [0, 0],
    };
    let out = CMd4cResult {
        headings: std::ptr::null_mut(),
        links: std::ptr::null_mut(),
        code_spans: std::ptr::null_mut(),
        tasks: std::ptr::null_mut(),
        embeds: std::ptr::null_mut(),
        callouts: std::ptr::null_mut(),
        block_refs: std::ptr::null_mut(),
        query_blocks: std::ptr::null_mut(),
        link_definitions: std::ptr::null_mut(),
        properties: std::ptr::null_mut(),
        xml_tags: &xml_tag as *const _ as *mut _,
        text_blob: blob.as_ptr(),
        headings_count: 0,
        links_count: 0,
        code_spans_count: 0,
        tasks_count: 0,
        embeds_count: 0,
        callouts_count: 0,
        block_refs_count: 0,
        query_blocks_count: 0,
        link_definitions_count: 0,
        properties_count: 0,
        xml_tags_count: 1,
        text_blob_len: blob.len() as u32,
    };
    let result = convert_result(&out).unwrap();
    assert_eq!(result.xml_tags.len(), 1);
    assert_eq!(result.xml_tags[0].tag_name, "div");
    assert_eq!(result.xml_tags[0].raw_html, "raw html here");
    assert_eq!(result.xml_tags[0].source_offset, 0);
    assert_eq!(result.xml_tags[0].end_offset, 20);
    assert!(!result.xml_tags[0].is_self_closing);
    assert!(result.xml_tags[0].is_unclosed);
}

#[test]
fn test_extract_inline_xml_tags_not_extracted() {
    // Inline HTML (<tag>content</tag> on one line) is NOT extracted.
    // md4c's inline HTML content may point to internal buffers, not source text.
    let input = "<agent>content</agent>\n\n<goal>win</goal>\n";
    let result = extract_md4c(input).unwrap();
    assert_eq!(
        result.xml_tags.len(),
        0,
        "inline HTML tags should not be extracted"
    );
}
