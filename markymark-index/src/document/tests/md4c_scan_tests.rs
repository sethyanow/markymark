//! md4c-based scan construction tests (Md4cScanBackend).

use super::*;
use markymark_core::scanner::{Md4cScanBackend, ZigScanBackend};

fn build_index_from_md4c_scan(source: &str) -> DocumentIndex {
    let backend = Md4cScanBackend;
    DocumentIndex::from_scan(source, &backend)
}

#[test]
fn test_md4c_from_scan_single_heading() {
    let index = build_index_from_md4c_scan("# Hello\n");
    assert_eq!(index.headings().len(), 1);
    assert_eq!(index.headings()[0].text, "Hello");
    assert_eq!(index.headings()[0].level, 1);
    assert_eq!(index.headings()[0].slug, "hello");
}

#[test]
fn test_md4c_from_scan_mixed_links() {
    let index =
        build_index_from_md4c_scan("See [example](https://example.com) and [[Wiki Page]]\n");
    assert_eq!(index.markdown_links().len(), 1);
    assert_eq!(index.markdown_links()[0].text, "example");
    assert_eq!(index.markdown_links()[0].url, "https://example.com");
    assert_eq!(index.wiki_links().len(), 1);
    assert_eq!(index.wiki_links()[0].target, "Wiki Page");
}

#[test]
fn test_md4c_parity_headings() {
    let text = "# First\n\n## Second\n\n### Third\n";
    let zig_backend = ZigScanBackend;
    let md4c_backend = Md4cScanBackend;
    let zig_idx = DocumentIndex::from_scan(text, &zig_backend);
    let md4c_idx = DocumentIndex::from_scan(text, &md4c_backend);

    assert_eq!(zig_idx.headings().len(), md4c_idx.headings().len());
    for (z, m) in zig_idx.headings().iter().zip(md4c_idx.headings().iter()) {
        assert_eq!(z.text, m.text, "heading text mismatch");
        assert_eq!(z.level, m.level, "heading level mismatch");
        assert_eq!(z.slug, m.slug, "heading slug mismatch");
    }
}

// ── Task/Embed from_scan tests (marky-bmu9) ──────────────────────

#[test]
fn test_md4c_from_scan_tasks() {
    let index = build_index_from_md4c_scan("- [ ] Todo\n- [x] Done\n");
    let tasks = index.tasks();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].state, "unchecked");
    assert_eq!(tasks[0].text, "Todo");
    assert_eq!(tasks[1].state, "checked");
    assert_eq!(tasks[1].text, "Done");
}

#[test]
fn test_md4c_from_scan_embeds() {
    let index = build_index_from_md4c_scan("![[target]]\n");
    let embeds = index.embeds();
    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].target, "target");
}

#[test]
fn test_md4c_from_scan_checkbox_parity() {
    // Verify from_scan task count matches from_ast for checkbox-only input.
    // md4c doesn't surface marker tasks (TODO/DONE), so use checkbox-only input.
    let text = "- [x] Done\n- [ ] Todo\n";
    let md4c_idx = build_index_from_md4c_scan(text);
    let ast_idx = build_index(text);
    assert_eq!(
        md4c_idx.tasks().len(),
        ast_idx.tasks().len(),
        "checkbox parity: md4c={}, ast={}",
        md4c_idx.tasks().len(),
        ast_idx.tasks().len(),
    );
}
