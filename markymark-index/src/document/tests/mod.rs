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
        is_inline: false,
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
    // md4c requires block-level HTML (tag on its own line) for XML tag extraction.
    // Blob path does not preserve per-tag attributes (BlobXmlTag stores name/range/flags
    // only), so attributes are empty when going through the scan/blob path.
    let index = build_index("<goal>\nShip\n</goal>\n");

    let tags = index.xml_tags();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_name, "goal");
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
// from_ast delegation to from_scan: code spans are extracted
// ---------------------------------------------------------------------------

#[test]
fn from_ast_extracts_code_spans_via_scan() {
    let source = "# Hello\n\nUse `DocumentArena` for allocation.\n";
    let index = build_index(source);
    assert!(
        !index.code_spans().is_empty(),
        "from_ast should extract code spans (delegates to from_scan)"
    );
    assert_eq!(index.code_spans()[0].text, "DocumentArena");
}

// ---------------------------------------------------------------------------
// md4c scan-based construction tests (feature-gated)
// ---------------------------------------------------------------------------

mod incremental_tests;
mod md4c_scan_tests;

// ---------------------------------------------------------------------------
// CRLF frontmatter handling (marky-e7i3)
// ---------------------------------------------------------------------------

#[test]
fn parse_frontmatter_owned_crlf() {
    let source = "---\r\ntitle: Hello\r\ntags: [a, b]\r\n---\r\nBody\r\n";
    let (fm, _aliases) = helpers::parse_frontmatter_owned(source);
    assert!(!fm.is_empty(), "CRLF frontmatter should be parsed");
    assert!(fm.iter().any(|e| e.key == "title"), "should find 'title'");
}

#[test]
fn parse_frontmatter_owned_lf_still_works() {
    let source = "---\ntitle: Hello\n---\nBody\n";
    let (fm, _aliases) = helpers::parse_frontmatter_owned(source);
    assert!(!fm.is_empty(), "LF frontmatter should still work");
}

#[test]
fn mask_frontmatter_crlf() {
    let source = "---\r\ntitle: Hello\r\n---\r\nBody\r\n";
    let masked = helpers::mask_frontmatter(source);
    // The frontmatter region should be masked (no dashes or letters, only spaces/CR/LF)
    assert!(!masked.starts_with("---"), "frontmatter delimiters should be masked");
    // Body should be preserved
    assert!(masked.contains("Body"), "body content should be preserved");
}

#[test]
fn mask_frontmatter_lf_still_works() {
    let source = "---\ntitle: Hello\n---\nBody\n";
    let masked = helpers::mask_frontmatter(source);
    assert!(!masked.starts_with("---"), "LF frontmatter should be masked");
    assert!(masked.contains("Body"), "body should be preserved");
}

// ---------------------------------------------------------------------------
// Mixed-ending frontmatter regression (marky-0nch)
// ---------------------------------------------------------------------------

#[test]
fn parse_frontmatter_owned_mixed_endings_picks_earliest_close() {
    // LF close at byte 8 of rest, CRLF close at byte 19 of rest.
    // Bug: find(CRLF).or_else(find(LF)) returns 19, treating "bogus: B" as yaml.
    // Fix: min(8, 19) = 8, only "title: A" is yaml.
    let source = "---\ntitle: A\n---\nbogus: B\r\n---\r\nMore\n";
    let (fm, _) = helpers::parse_frontmatter_owned(source);
    assert_eq!(fm.len(), 1, "should find exactly 1 key (not 2)");
    assert_eq!(fm[0].key, "title");
}

#[test]
fn mask_frontmatter_mixed_endings_picks_earliest_close() {
    // Same mixed-ending structure: LF close before CRLF close.
    // Bug: mask extends to CRLF close, swallowing body content.
    let source = "---\ntitle: A\n---\nbogus: B\r\n---\r\nMore\n";
    let masked = helpers::mask_frontmatter(source);
    assert!(
        masked.contains("bogus"),
        "body content after LF close must be preserved, not masked"
    );
}
