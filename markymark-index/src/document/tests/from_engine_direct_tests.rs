//! Parity test for `from_engine_result_direct` — verifies the direct arena decode path
//! produces identical DocumentIndex content as the old `to_extraction` + `from_engine_result_with_frontmatter` path.

use super::*;
use markymark_kernels::engine::DocumentEngine;

/// Comprehensive parity test covering ALL element types: headings, wiki links
/// (with/without alias and heading anchor), markdown links (with/without anchor),
/// tags, code spans, block_ids, tasks (checked + unchecked), embeds, callouts
/// (with/without title), block_refs, query_blocks, link_definitions (with/without
/// title), properties, xml_tags.
#[test]
fn test_from_engine_result_direct_parity() {
    let source = "\
---
title: Test Doc
tags: [rust, markdown]
aliases: [alias1, alias2]
---

# Main Heading

Some text with a [[Target Page]] and [[Other#section|display text]] link.

## Sub Heading

A [markdown link](https://example.com) and [anchored](https://example.com#frag).

#tag1 #tag2

Use `inline_code` and `another_span` in text.

^block-id-1

- [ ] unchecked task
- [x] checked task

![[embed-target]]

> [!note] My Callout Title
> Callout content here.

> [!warning]
> No title callout.

((550e8400-e29b-41d4-a716-446655440000))

```query
from: [[Target Page]]
```

[ref-label]: https://ref.example.com \"Ref Title\"
[no-title]: https://no-title.example.com

status:: active
priority:: high

<goal>
Ship it
</goal>
";

    let (fm, aliases) = helpers::parse_frontmatter_owned(source);
    let masked = helpers::mask_frontmatter(source);
    let engine = DocumentEngine::new(&masked).expect("engine create failed");
    let result = engine.get_result().expect("get_result failed");

    // Old path: EngineResult → EngineExtraction → from_engine_result_with_frontmatter
    let extraction = result.to_extraction().expect("to_extraction failed");
    let old_index = DocumentIndex::from_engine_result_with_frontmatter(
        &extraction,
        fm.clone(),
        aliases.clone(),
    );

    // New path: EngineResult → from_engine_result_direct (bypasses EngineExtraction)
    let new_index = DocumentIndex::from_engine_result_direct(&result, fm, aliases)
        .expect("from_engine_result_direct failed");

    // --- Compare ALL element types ---

    // Headings
    assert_eq!(
        old_index.headings().len(),
        new_index.headings().len(),
        "headings count mismatch"
    );
    for (i, (old, new)) in old_index
        .headings()
        .iter()
        .zip(new_index.headings().iter())
        .enumerate()
    {
        assert_eq!(old.text, new.text, "heading[{i}].text");
        assert_eq!(old.slug, new.slug, "heading[{i}].slug");
        assert_eq!(old.level, new.level, "heading[{i}].level");
        assert_eq!(old.range, new.range, "heading[{i}].range");
    }

    // Wiki links
    assert_eq!(
        old_index.wiki_links().len(),
        new_index.wiki_links().len(),
        "wiki_links count mismatch"
    );
    for (i, (old, new)) in old_index
        .wiki_links()
        .iter()
        .zip(new_index.wiki_links().iter())
        .enumerate()
    {
        assert_eq!(old.target, new.target, "wiki_link[{i}].target");
        assert_eq!(old.alias, new.alias, "wiki_link[{i}].alias");
        assert_eq!(old.heading, new.heading, "wiki_link[{i}].heading");
        assert_eq!(old.range, new.range, "wiki_link[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "wiki_link[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "wiki_link[{i}].end_byte");
    }

    // Markdown links
    assert_eq!(
        old_index.markdown_links().len(),
        new_index.markdown_links().len(),
        "markdown_links count mismatch"
    );
    for (i, (old, new)) in old_index
        .markdown_links()
        .iter()
        .zip(new_index.markdown_links().iter())
        .enumerate()
    {
        assert_eq!(old.text, new.text, "md_link[{i}].text");
        assert_eq!(old.url, new.url, "md_link[{i}].url");
        assert_eq!(old.anchor, new.anchor, "md_link[{i}].anchor");
        assert_eq!(old.range, new.range, "md_link[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "md_link[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "md_link[{i}].end_byte");
    }

    // Tags
    assert_eq!(
        old_index.tags().len(),
        new_index.tags().len(),
        "tags count mismatch"
    );
    for (i, (old, new)) in old_index
        .tags()
        .iter()
        .zip(new_index.tags().iter())
        .enumerate()
    {
        assert_eq!(old.name, new.name, "tag[{i}].name");
    }

    // Code spans
    assert_eq!(
        old_index.code_spans().len(),
        new_index.code_spans().len(),
        "code_spans count mismatch"
    );
    for (i, (old, new)) in old_index
        .code_spans()
        .iter()
        .zip(new_index.code_spans().iter())
        .enumerate()
    {
        assert_eq!(old.text, new.text, "code_span[{i}].text");
        assert_eq!(old.range, new.range, "code_span[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "code_span[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "code_span[{i}].end_byte");
    }

    // Block IDs
    let old_bids: Vec<&str> = old_index.block_ids().collect();
    let new_bids: Vec<&str> = new_index.block_ids().collect();
    assert_eq!(old_bids, new_bids, "block_ids mismatch");

    // Tasks
    assert_eq!(
        old_index.tasks().len(),
        new_index.tasks().len(),
        "tasks count mismatch"
    );
    for (i, (old, new)) in old_index
        .tasks()
        .iter()
        .zip(new_index.tasks().iter())
        .enumerate()
    {
        assert_eq!(old.state, new.state, "task[{i}].state");
        assert_eq!(old.text, new.text, "task[{i}].text");
        assert_eq!(old.range, new.range, "task[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "task[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "task[{i}].end_byte");
    }

    // Embeds
    assert_eq!(
        old_index.embeds().len(),
        new_index.embeds().len(),
        "embeds count mismatch"
    );
    for (i, (old, new)) in old_index
        .embeds()
        .iter()
        .zip(new_index.embeds().iter())
        .enumerate()
    {
        assert_eq!(old.target, new.target, "embed[{i}].target");
        assert_eq!(old.range, new.range, "embed[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "embed[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "embed[{i}].end_byte");
    }

    // Callouts
    assert_eq!(
        old_index.callouts().len(),
        new_index.callouts().len(),
        "callouts count mismatch"
    );
    for (i, (old, new)) in old_index
        .callouts()
        .iter()
        .zip(new_index.callouts().iter())
        .enumerate()
    {
        assert_eq!(old.callout_type, new.callout_type, "callout[{i}].type");
        assert_eq!(old.title, new.title, "callout[{i}].title");
        assert_eq!(old.range, new.range, "callout[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "callout[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "callout[{i}].end_byte");
    }

    // Block refs
    assert_eq!(
        old_index.block_refs().len(),
        new_index.block_refs().len(),
        "block_refs count mismatch"
    );
    for (i, (old, new)) in old_index
        .block_refs()
        .iter()
        .zip(new_index.block_refs().iter())
        .enumerate()
    {
        assert_eq!(old.uuid, new.uuid, "block_ref[{i}].uuid");
        assert_eq!(old.range, new.range, "block_ref[{i}].range");
    }

    // Query blocks
    assert_eq!(
        old_index.query_blocks().len(),
        new_index.query_blocks().len(),
        "query_blocks count mismatch"
    );
    for (i, (old, new)) in old_index
        .query_blocks()
        .iter()
        .zip(new_index.query_blocks().iter())
        .enumerate()
    {
        assert_eq!(old.query, new.query, "query_block[{i}].query");
        assert_eq!(old.range, new.range, "query_block[{i}].range");
        assert_eq!(
            old.start_byte, new.start_byte,
            "query_block[{i}].start_byte"
        );
        assert_eq!(old.end_byte, new.end_byte, "query_block[{i}].end_byte");
    }

    // Link definitions
    assert_eq!(
        old_index.link_definitions().len(),
        new_index.link_definitions().len(),
        "link_definitions count mismatch"
    );
    for (i, (old, new)) in old_index
        .link_definitions()
        .iter()
        .zip(new_index.link_definitions().iter())
        .enumerate()
    {
        assert_eq!(old.label, new.label, "link_def[{i}].label");
        assert_eq!(old.url, new.url, "link_def[{i}].url");
        assert_eq!(old.title, new.title, "link_def[{i}].title");
        assert_eq!(old.range, new.range, "link_def[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "link_def[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "link_def[{i}].end_byte");
    }

    // Properties
    assert_eq!(
        old_index.properties().len(),
        new_index.properties().len(),
        "properties count mismatch"
    );
    for (i, (old, new)) in old_index
        .properties()
        .iter()
        .zip(new_index.properties().iter())
        .enumerate()
    {
        assert_eq!(old.key, new.key, "property[{i}].key");
    }

    // XML tags
    assert_eq!(
        old_index.xml_tags().len(),
        new_index.xml_tags().len(),
        "xml_tags count mismatch"
    );
    for (i, (old, new)) in old_index
        .xml_tags()
        .iter()
        .zip(new_index.xml_tags().iter())
        .enumerate()
    {
        assert_eq!(old.tag_name, new.tag_name, "xml_tag[{i}].tag_name");
        assert_eq!(
            old.is_self_closing, new.is_self_closing,
            "xml_tag[{i}].is_self_closing"
        );
        assert_eq!(old.is_unclosed, new.is_unclosed, "xml_tag[{i}].is_unclosed");
        assert_eq!(old.is_inline, new.is_inline, "xml_tag[{i}].is_inline");
        assert_eq!(old.range, new.range, "xml_tag[{i}].range");
        assert_eq!(old.start_byte, new.start_byte, "xml_tag[{i}].start_byte");
        assert_eq!(old.end_byte, new.end_byte, "xml_tag[{i}].end_byte");
    }

    // Frontmatter
    assert_eq!(
        old_index.frontmatter().len(),
        new_index.frontmatter().len(),
        "frontmatter count mismatch"
    );
    for (i, (old, new)) in old_index
        .frontmatter()
        .iter()
        .zip(new_index.frontmatter().iter())
        .enumerate()
    {
        assert_eq!(old.key, new.key, "frontmatter[{i}].key");
    }

    // Aliases
    assert_eq!(old_index.aliases(), new_index.aliases(), "aliases mismatch");

    // TOC
    assert_eq!(
        old_index.toc().len(),
        new_index.toc().len(),
        "toc count mismatch"
    );
    for (i, (old, new)) in old_index
        .toc()
        .iter()
        .zip(new_index.toc().iter())
        .enumerate()
    {
        assert_eq!(old.text, new.text, "toc[{i}].text");
        assert_eq!(old.slug, new.slug, "toc[{i}].slug");
        assert_eq!(old.level, new.level, "toc[{i}].level");
        assert_eq!(old.depth, new.depth, "toc[{i}].depth");
    }
}

/// Adversarial: empty document — zero typed slices, minimal text_blob.
/// Verifies try_new succeeds when no blob reads are needed.
#[test]
fn test_direct_empty_document() {
    let source = "";
    let engine = DocumentEngine::new(source).expect("engine create");
    let result = engine.get_result().expect("get_result");
    let index = DocumentIndex::from_engine_result_direct(&result, vec![], vec![])
        .expect("direct should succeed on empty input");
    assert_eq!(index.headings().len(), 0);
    assert_eq!(index.wiki_links().len(), 0);
    assert_eq!(index.tags().len(), 0);
    assert_eq!(index.markdown_links().len(), 0);
    assert_eq!(index.code_spans().len(), 0);
    assert_eq!(index.tasks().len(), 0);
    assert_eq!(index.embeds().len(), 0);
    assert_eq!(index.callouts().len(), 0);
    assert_eq!(index.block_refs().len(), 0);
    assert_eq!(index.query_blocks().len(), 0);
    assert_eq!(index.link_definitions().len(), 0);
    assert_eq!(index.properties().len(), 0);
    assert_eq!(index.xml_tags().len(), 0);
    assert_eq!(index.frontmatter().len(), 0);
    assert_eq!(index.aliases().len(), 0);
}

/// Adversarial: multi-byte UTF-8 in headings, wiki links, tags.
/// Verifies read_blob_str correctly handles multi-byte char boundaries
/// when borrowing from owner.text_blob. Uses parity against old path.
#[test]
fn test_direct_multibyte_utf8_parity() {
    // Use format known to work with engine: heading with space, wiki link, tag
    let source = "# 日本語の見出し\n\nSome text with [[Ñoño]] and [[café#sección|表示]]\n\n#タグ名 more text\n\nUse `コード` here.\n";
    let engine = DocumentEngine::new(source).expect("engine create");
    let result = engine.get_result().expect("get_result");
    let extraction = result.to_extraction().expect("extraction");

    let old = DocumentIndex::from_engine_result_with_frontmatter(&extraction, vec![], vec![]);
    let new = DocumentIndex::from_engine_result_direct(&result, vec![], vec![])
        .expect("direct should handle multibyte");

    // Headings — at minimum the h1 should be extracted
    assert_eq!(old.headings().len(), new.headings().len(), "headings count");
    assert!(new.headings().len() >= 1, "should have at least one heading");
    assert_eq!(old.headings()[0].text, new.headings()[0].text);
    assert_eq!(new.headings()[0].text, "日本語の見出し");

    // Wiki links — parity between old and new, however many the engine extracts
    assert_eq!(old.wiki_links().len(), new.wiki_links().len(), "wiki count");
    for (i, (o, n)) in old.wiki_links().iter().zip(new.wiki_links()).enumerate() {
        assert_eq!(o.target, n.target, "wiki[{i}].target");
        assert_eq!(o.alias, n.alias, "wiki[{i}].alias");
        assert_eq!(o.heading, n.heading, "wiki[{i}].heading");
    }

    // Tags — parity
    assert_eq!(old.tags().len(), new.tags().len(), "tags count");
    for (i, (o, n)) in old.tags().iter().zip(new.tags()).enumerate() {
        assert_eq!(o.name, n.name, "tag[{i}].name");
    }

    // Code spans — parity
    assert_eq!(old.code_spans().len(), new.code_spans().len(), "code_spans count");
    for (i, (o, n)) in old.code_spans().iter().zip(new.code_spans()).enumerate() {
        assert_eq!(o.text, n.text, "code_span[{i}].text");
    }
}

/// Adversarial: second run — two indexes from same EngineResult.
/// Verifies owner.text_blob is an independent copy (no shared mutable state).
#[test]
fn test_direct_second_run_independent() {
    let source = "# Heading\n\n[[Link]]\n\n#tag\n";
    let engine = DocumentEngine::new(source).expect("engine create");
    let result = engine.get_result().expect("get_result");

    let idx1 = DocumentIndex::from_engine_result_direct(&result, vec![], vec![])
        .expect("first construction");
    let idx2 = DocumentIndex::from_engine_result_direct(&result, vec![], vec![])
        .expect("second construction");

    // Both should produce identical content
    assert_eq!(idx1.headings()[0].text, idx2.headings()[0].text);
    assert_eq!(idx1.wiki_links()[0].target, idx2.wiki_links()[0].target);
    assert_eq!(idx1.tags()[0].name, idx2.tags()[0].name);

    // But they should be independent allocations (different pointers)
    let ptr1 = idx1.headings()[0].text.as_ptr();
    let ptr2 = idx2.headings()[0].text.as_ptr();
    assert_ne!(ptr1, ptr2, "text fields should be independent copies in separate owners");
}
