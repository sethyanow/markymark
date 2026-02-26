// Parity tests — from_engine_result vs from_blob.

use super::super::super::DocumentIndex;
use super::blob_for;
use markymark_kernels::engine::DocumentEngine;

fn index_from_engine(text: &str) -> DocumentIndex {
    let engine = DocumentEngine::new(text).expect("engine creation failed");
    let result = engine.get_result().expect("get_result failed");
    let extraction = result
        .to_extraction()
        .expect("convert_engine_result failed");
    DocumentIndex::from_engine_result(&extraction)
}

fn assert_index_parity(lhs: &DocumentIndex, rhs: &DocumentIndex) {
    assert_eq!(lhs.headings().len(), rhs.headings().len(), "headings count");
    for (l, r) in lhs.headings().iter().zip(rhs.headings()) {
        assert_eq!(l.text, r.text, "heading text");
        assert_eq!(l.slug, r.slug, "heading slug");
        assert_eq!(l.level, r.level, "heading level");
        assert_eq!(l.range, r.range, "heading range");
    }

    assert_eq!(lhs.wiki_links().len(), rhs.wiki_links().len(), "wiki count");
    for (l, r) in lhs.wiki_links().iter().zip(rhs.wiki_links()) {
        assert_eq!(l.target, r.target, "wiki target");
        assert_eq!(l.alias, r.alias, "wiki alias");
        assert_eq!(l.heading, r.heading, "wiki heading");
        assert_eq!(l.range, r.range, "wiki range");
        assert_eq!(l.start_byte, r.start_byte, "wiki start_byte");
        assert_eq!(l.end_byte, r.end_byte, "wiki end_byte");
    }

    assert_eq!(
        lhs.markdown_links().len(),
        rhs.markdown_links().len(),
        "markdown count"
    );
    for (l, r) in lhs.markdown_links().iter().zip(rhs.markdown_links()) {
        assert_eq!(l.text, r.text, "markdown text");
        assert_eq!(l.url, r.url, "markdown url");
        assert_eq!(l.anchor, r.anchor, "markdown anchor");
        assert_eq!(l.range, r.range, "markdown range");
        assert_eq!(l.start_byte, r.start_byte, "markdown start_byte");
        assert_eq!(l.end_byte, r.end_byte, "markdown end_byte");
    }

    assert_eq!(lhs.tags().len(), rhs.tags().len(), "tags count");
    for (l, r) in lhs.tags().iter().zip(rhs.tags()) {
        assert_eq!(l.name, r.name, "tag name");
    }

    assert_eq!(
        lhs.code_spans().len(),
        rhs.code_spans().len(),
        "code span count"
    );
    for (l, r) in lhs.code_spans().iter().zip(rhs.code_spans()) {
        assert_eq!(l.text, r.text, "code span text");
        assert_eq!(l.range, r.range, "code span range");
        assert_eq!(l.start_byte, r.start_byte, "code span start_byte");
        assert_eq!(l.end_byte, r.end_byte, "code span end_byte");
    }

    assert_eq!(lhs.tasks().len(), rhs.tasks().len(), "task count");
    for (l, r) in lhs.tasks().iter().zip(rhs.tasks()) {
        assert_eq!(l.state, r.state, "task state");
        assert_eq!(l.text, r.text, "task text");
        assert_eq!(l.range, r.range, "task range");
        assert_eq!(l.start_byte, r.start_byte, "task start_byte");
        assert_eq!(l.end_byte, r.end_byte, "task end_byte");
    }

    assert_eq!(lhs.embeds().len(), rhs.embeds().len(), "embed count");
    for (l, r) in lhs.embeds().iter().zip(rhs.embeds()) {
        assert_eq!(l.target, r.target, "embed target");
        assert_eq!(l.range, r.range, "embed range");
        assert_eq!(l.start_byte, r.start_byte, "embed start_byte");
        assert_eq!(l.end_byte, r.end_byte, "embed end_byte");
    }

    assert_eq!(lhs.callouts().len(), rhs.callouts().len(), "callout count");
    for (l, r) in lhs.callouts().iter().zip(rhs.callouts()) {
        assert_eq!(l.callout_type, r.callout_type, "callout type");
        assert_eq!(l.title, r.title, "callout title");
        assert_eq!(l.range, r.range, "callout range");
        assert_eq!(l.start_byte, r.start_byte, "callout start_byte");
        assert_eq!(l.end_byte, r.end_byte, "callout end_byte");
    }

    assert_eq!(
        lhs.block_refs().len(),
        rhs.block_refs().len(),
        "block_ref count"
    );
    for (l, r) in lhs.block_refs().iter().zip(rhs.block_refs()) {
        assert_eq!(l.uuid, r.uuid, "block_ref uuid");
        assert_eq!(l.range, r.range, "block_ref range");
    }

    assert_eq!(
        lhs.query_blocks().len(),
        rhs.query_blocks().len(),
        "query_blocks count"
    );
    for (l, r) in lhs.query_blocks().iter().zip(rhs.query_blocks()) {
        assert_eq!(l.query, r.query, "query text");
        assert_eq!(l.range, r.range, "query range");
        assert_eq!(l.start_byte, r.start_byte, "query start_byte");
        assert_eq!(l.end_byte, r.end_byte, "query end_byte");
    }

    assert_eq!(
        lhs.link_definitions().len(),
        rhs.link_definitions().len(),
        "link_definitions count"
    );
    for (l, r) in lhs.link_definitions().iter().zip(rhs.link_definitions()) {
        assert_eq!(l.label, r.label, "link_def label");
        assert_eq!(l.url, r.url, "link_def url");
        assert_eq!(l.title, r.title, "link_def title");
        assert_eq!(l.range, r.range, "link_def range");
        assert_eq!(l.start_byte, r.start_byte, "link_def start_byte");
        assert_eq!(l.end_byte, r.end_byte, "link_def end_byte");
    }

    assert_eq!(
        lhs.properties().len(),
        rhs.properties().len(),
        "properties count"
    );
    for (l, r) in lhs.properties().iter().zip(rhs.properties()) {
        assert_eq!(l.key, r.key, "property key");
        match (&l.value, &r.value) {
            (
                super::super::super::PropertyValueEntry::String(ls),
                super::super::super::PropertyValueEntry::String(rs),
            ) => assert_eq!(ls, rs, "property string value"),
            (
                super::super::super::PropertyValueEntry::List(ll),
                super::super::super::PropertyValueEntry::List(rl),
            ) => assert_eq!(ll, rl, "property list value"),
            (
                super::super::super::PropertyValueEntry::PageRef(lp),
                super::super::super::PropertyValueEntry::PageRef(rp),
            ) => assert_eq!(lp, rp, "property pageref value"),
            _ => panic!("property value variant mismatch"),
        }
    }

    assert_eq!(lhs.xml_tags().len(), rhs.xml_tags().len(), "xml tags count");
    for (l, r) in lhs.xml_tags().iter().zip(rhs.xml_tags()) {
        assert_eq!(l.tag_name, r.tag_name, "xml tag name");
        assert_eq!(l.is_self_closing, r.is_self_closing, "xml self closing");
        assert_eq!(l.is_unclosed, r.is_unclosed, "xml unclosed");
        assert_eq!(l.is_inline, r.is_inline, "xml inline");
        assert_eq!(l.range, r.range, "xml range");
        assert_eq!(l.start_byte, r.start_byte, "xml start_byte");
        assert_eq!(l.end_byte, r.end_byte, "xml end_byte");
    }

    let mut lhs_ids: Vec<&str> = lhs.block_ids().collect();
    let mut rhs_ids: Vec<&str> = rhs.block_ids().collect();
    lhs_ids.sort_unstable();
    rhs_ids.sort_unstable();
    assert_eq!(lhs_ids, rhs_ids, "block id set mismatch");

    for id in lhs_ids {
        let l = lhs.block_by_id(id).expect("lhs block missing");
        let r = rhs.block_by_id(id).expect("rhs block missing");
        assert_eq!(l.id, r.id, "block id text mismatch");
        assert_eq!(l.range, r.range, "block range mismatch");
        assert_eq!(l.start_byte, r.start_byte, "block start_byte mismatch");
        assert_eq!(l.end_byte, r.end_byte, "block end_byte mismatch");
    }
}

#[test]
fn test_from_engine_result_parity_empty_doc() {
    let text = "";
    let blob = blob_for(text);
    let blob_idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let engine_idx = index_from_engine(text);
    assert_index_parity(&blob_idx, &engine_idx);
}

#[test]
fn test_from_engine_result_parity_all_element_types() {
    let text = concat!(
        "# Heading One\n",
        "## Heading One\n",
        "\n",
        "[[Page#Section|Alias]]\n",
        "[md](doc.md#anchor)\n",
        "line with #tag and ^block-one\n",
        "inline `code` span\n",
        "- [x] Done task\n",
        "![[embed-target]]\n",
        "> [!note] Title\n",
        "> callout body\n",
        "((a1b2c3d4-e5f6-7890-abcd-ef1234567890))\n",
        "{{query (task TODO)}}\n",
        "[ref]: https://example.com \"Title\"\n",
        "key:: [[Page Ref]]\n",
        "\n",
        "<agent>\n",
        "\n",
        "body\n",
        "\n",
        "</agent>\n",
    );

    let blob = blob_for(text);
    let blob_idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let engine_idx = index_from_engine(text);
    assert_index_parity(&blob_idx, &engine_idx);
}

#[test]
fn test_from_engine_result_parity_unicode_and_nullable_fields() {
    let text = concat!(
        "# 🎉 Party\n",
        "[[ページ#見出し|表示]]\n",
        "[参照](docs/日本語.md#章)\n",
        "`コード`\n",
        "> [!tip]\n",
        "> no title\n",
        "[plain]: https://example.org\n",
    );

    let blob = blob_for(text);
    let blob_idx = DocumentIndex::from_blob(&blob).expect("from_blob failed");
    let engine_idx = index_from_engine(text);
    assert_index_parity(&blob_idx, &engine_idx);
}
