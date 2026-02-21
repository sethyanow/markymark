use std::path::PathBuf;

use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;

/// Helper: parse markdown source and build a DocumentIndex.
fn index_from(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse");
    DocumentIndex::from_ast(ast)
}

/// Helper: create a file:// URI from a filename.
fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{}", name)))
}

// ---------------------------------------------------------------------------
// Empty realm
// ---------------------------------------------------------------------------

#[test]
fn test_empty_realm_index() {
    let realm = RealmIndex::new();
    assert_eq!(
        realm.document_count(),
        0,
        "new realm should have 0 documents"
    );
}

// ---------------------------------------------------------------------------
// Adding documents
// ---------------------------------------------------------------------------

#[test]
fn test_add_document() {
    let mut realm = RealmIndex::new();
    let doc_uri = uri("notes.md");
    let idx = index_from("# Introduction\n\nSome content.\n\n## Details");

    realm.add_document(doc_uri.clone(), idx);

    assert_eq!(
        realm.document_count(),
        1,
        "realm should have 1 document after add"
    );

    // Should be able to look up headings from that document
    let results = realm.lookup_heading("introduction");
    assert_eq!(results.len(), 1, "should find the 'introduction' heading");
    assert_eq!(results[0].1.text, "Introduction");
}

// ---------------------------------------------------------------------------
// Global heading lookup
// ---------------------------------------------------------------------------

#[test]
fn test_global_heading_lookup() {
    let mut realm = RealmIndex::new();

    let uri_a = uri("page-a.md");
    let idx_a = index_from("# Alpha\n\n## Beta");
    realm.add_document(uri_a.clone(), idx_a);

    let uri_b = uri("page-b.md");
    let idx_b = index_from("# Gamma\n\n## Delta");
    realm.add_document(uri_b.clone(), idx_b);

    // Look up "alpha" - should only be in page-a
    let results = realm.lookup_heading("alpha");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.as_str(), uri_a.as_str());

    // Look up "delta" - should only be in page-b
    let results = realm.lookup_heading("delta");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.as_str(), uri_b.as_str());

    // Look up nonexistent heading
    let results = realm.lookup_heading("nonexistent");
    assert!(
        results.is_empty(),
        "nonexistent slug should return empty vec"
    );
}

// ---------------------------------------------------------------------------
// Global block lookup
// ---------------------------------------------------------------------------

#[test]
fn test_global_block_lookup() {
    let mut realm = RealmIndex::new();

    let doc_uri = uri("blocks.md");
    let idx = index_from("A paragraph ^block-abc\n\nAnother paragraph ^block-def");
    realm.add_document(doc_uri.clone(), idx);

    // Look up existing block
    let result = realm.lookup_block("block-abc");
    assert!(result.is_some(), "should find block-abc");
    let (found_uri, block) = result.unwrap();
    assert_eq!(found_uri.as_str(), doc_uri.as_str());
    assert_eq!(block.id, "block-abc");

    // Look up second block
    let result = realm.lookup_block("block-def");
    assert!(result.is_some(), "should find block-def");

    // Look up nonexistent block
    let result = realm.lookup_block("nonexistent");
    assert!(result.is_none(), "nonexistent block should return None");
}

#[test]
fn test_block_lookup_prefers_first_inserted_doc_on_collision() {
    let mut realm = RealmIndex::new();
    let uri_a = uri("block-a.md");
    let uri_b = uri("block-b.md");

    realm.add_document(uri_a.clone(), index_from("Doc A line ^shared-block"));
    realm.add_document(uri_b.clone(), index_from("Doc B line ^shared-block"));

    let (resolved_uri, block) = realm
        .lookup_block("shared-block")
        .expect("shared block should resolve");
    assert_eq!(resolved_uri.as_str(), uri_a.as_str());
    assert_eq!(block.id, "shared-block");
}

// ---------------------------------------------------------------------------
// Global tag table
// ---------------------------------------------------------------------------

#[test]
fn test_global_tag_table() {
    let mut realm = RealmIndex::new();

    let uri_a = uri("tags-a.md");
    let idx_a = index_from("Content #rust and #programming here");
    realm.add_document(uri_a, idx_a);

    let uri_b = uri("tags-b.md");
    let idx_b = index_from("More #rust content and #design");
    realm.add_document(uri_b, idx_b);

    let counts = realm.tag_counts();

    // Find rust count - should appear in both docs
    let rust_count = counts.iter().find(|(name, _)| *name == "rust");
    assert!(rust_count.is_some(), "should have 'rust' tag");
    assert_eq!(
        rust_count.unwrap().1,
        2,
        "rust should appear in 2 documents"
    );

    // programming appears only in doc A
    let prog_count = counts.iter().find(|(name, _)| *name == "programming");
    assert!(prog_count.is_some(), "should have 'programming' tag");
    assert_eq!(
        prog_count.unwrap().1,
        1,
        "programming should appear in 1 document"
    );

    // design appears only in doc B
    let design_count = counts.iter().find(|(name, _)| *name == "design");
    assert!(design_count.is_some(), "should have 'design' tag");
    assert_eq!(
        design_count.unwrap().1,
        1,
        "design should appear in 1 document"
    );
}

// ---------------------------------------------------------------------------
// Remove document
// ---------------------------------------------------------------------------

#[test]
fn test_remove_document() {
    let mut realm = RealmIndex::new();

    let doc_uri = uri("removable.md");
    let idx = index_from("# Temporary\n\nSome text #ephemeral ^temp-block");
    realm.add_document(doc_uri.clone(), idx);

    assert_eq!(realm.document_count(), 1);
    assert_eq!(realm.lookup_heading("temporary").len(), 1);

    // Remove the document
    realm.remove_document(&doc_uri);

    assert_eq!(
        realm.document_count(),
        0,
        "should have 0 docs after removal"
    );
    assert!(
        realm.lookup_heading("temporary").is_empty(),
        "heading should be gone after removal"
    );
    assert!(
        realm.lookup_block("temp-block").is_none(),
        "block should be gone after removal"
    );
    assert!(
        realm.get_document(&doc_uri).is_none(),
        "document should not be retrievable after removal"
    );
}

// ---------------------------------------------------------------------------
// Heading collision across documents
// ---------------------------------------------------------------------------

#[test]
fn test_heading_collision_across_docs() {
    let mut realm = RealmIndex::new();

    // Both documents have a heading that slugifies to "introduction"
    let uri_a = uri("doc-a.md");
    let idx_a = index_from("# Introduction\n\nDoc A content");
    realm.add_document(uri_a.clone(), idx_a);

    let uri_b = uri("doc-b.md");
    let idx_b = index_from("# Introduction\n\nDoc B content");
    realm.add_document(uri_b.clone(), idx_b);

    let results = realm.lookup_heading("introduction");
    assert_eq!(
        results.len(),
        2,
        "both documents with same heading slug should be returned"
    );

    // Both URIs should be present in results
    let uris: Vec<&str> = results.iter().map(|(u, _)| u.as_str()).collect();
    assert!(uris.contains(&uri_a.as_str()));
    assert!(uris.contains(&uri_b.as_str()));
}

// ---------------------------------------------------------------------------
// Targeted removal preserves sibling document entries
// ---------------------------------------------------------------------------

#[test]
fn test_remove_document_preserves_sibling_cross_doc_entries() {
    let mut realm = RealmIndex::new();

    // Doc A: heading "shared", tag #rust, block ^shared-block
    let uri_a = uri("doc-a.md");
    let idx_a = index_from("# Shared\n\nContent #rust here ^shared-block");
    realm.add_document(uri_a.clone(), idx_a);

    // Doc B: heading "shared" (collision), tag #rust (shared), block ^only-b
    let uri_b = uri("doc-b.md");
    let idx_b = index_from("# Shared\n\nMore #rust content ^only-b");
    realm.add_document(uri_b.clone(), idx_b);

    // Verify both docs contribute to cross-doc indexes
    assert_eq!(realm.lookup_heading("shared").len(), 2);
    let rust_count = realm
        .tag_counts()
        .iter()
        .find(|(n, _)| n == "rust")
        .unwrap()
        .1;
    assert_eq!(rust_count, 2);

    // Remove doc A
    realm.remove_document(&uri_a);

    // Doc B's entries must survive
    let headings = realm.lookup_heading("shared");
    assert_eq!(headings.len(), 1, "only doc-b's heading should remain");
    assert_eq!(headings[0].0.as_str(), uri_b.as_str());

    let rust_count = realm
        .tag_counts()
        .iter()
        .find(|(n, _)| n == "rust")
        .unwrap()
        .1;
    assert_eq!(rust_count, 1, "only doc-b's #rust tag should remain");

    assert!(
        realm.lookup_block("shared-block").is_none(),
        "doc-a's block should be gone"
    );
    assert!(
        realm.lookup_block("only-b").is_some(),
        "doc-b's block should survive"
    );
}

#[test]
fn test_replace_document_via_add_cleans_old_entries() {
    let mut realm = RealmIndex::new();

    let doc_uri = uri("evolving.md");
    let idx_v1 = index_from("# Old Title\n\nOld content #deprecated ^old-block");
    realm.add_document(doc_uri.clone(), idx_v1);

    assert_eq!(realm.lookup_heading("old-title").len(), 1);
    assert!(realm.lookup_block("old-block").is_some());

    // Replace with new content (same URI)
    let idx_v2 = index_from("# New Title\n\nNew content #fresh ^new-block");
    realm.add_document(doc_uri.clone(), idx_v2);

    // Old entries gone
    assert!(
        realm.lookup_heading("old-title").is_empty(),
        "old heading should be removed on replace"
    );
    assert!(
        realm.lookup_block("old-block").is_none(),
        "old block should be removed on replace"
    );
    let has_deprecated = realm
        .tag_counts()
        .into_iter()
        .any(|(n, _)| n == "deprecated");
    assert!(!has_deprecated, "old tag should be removed on replace");

    // New entries present
    assert_eq!(realm.lookup_heading("new-title").len(), 1);
    assert!(realm.lookup_block("new-block").is_some());
    let has_fresh = realm.tag_counts().into_iter().any(|(n, _)| n == "fresh");
    assert!(has_fresh, "new tag should be present after replace");
}

// ---------------------------------------------------------------------------
// Document lookup by URI
// ---------------------------------------------------------------------------

#[test]
fn test_document_lookup_by_uri() {
    let mut realm = RealmIndex::new();

    let doc_uri = uri("my-notes.md");
    let idx = index_from("# My Notes\n\n## Section One\n\n## Section Two");
    realm.add_document(doc_uri.clone(), idx);

    // Look up the document
    let doc = realm.get_document(&doc_uri);
    assert!(doc.is_some(), "should find document by URI");

    let doc = doc.unwrap();
    assert_eq!(doc.headings().len(), 3, "document should have 3 headings");
    assert_eq!(doc.headings()[0].text, "My Notes");

    // Look up nonexistent document
    let missing_uri = uri("nonexistent.md");
    assert!(
        realm.get_document(&missing_uri).is_none(),
        "nonexistent URI should return None"
    );
}

// ---------------------------------------------------------------------------
// String interning regression tests (marky-2yzz)
// ---------------------------------------------------------------------------

#[test]
fn test_interned_slug_dedup_cross_doc() {
    // Two documents with identical heading slugs should share the same
    // interned key internally. Verify both are accessible via lookup.
    let mut realm = RealmIndex::new();

    let uri_a = uri("intern-a.md");
    let uri_b = uri("intern-b.md");
    realm.add_document(uri_a.clone(), index_from("# Intro\n\nDoc A"));
    realm.add_document(uri_b.clone(), index_from("# Intro\n\nDoc B"));

    let results = realm.lookup_heading("intro");
    assert_eq!(
        results.len(),
        2,
        "both docs should contribute to slug 'intro'"
    );

    let uris: Vec<&str> = results.iter().map(|(u, _)| u.as_str()).collect();
    assert!(uris.contains(&uri_a.as_str()));
    assert!(uris.contains(&uri_b.as_str()));
}

#[test]
fn test_remove_then_readd_same_content() {
    // Interner retains old Spur values; re-adding same content must work.
    let mut realm = RealmIndex::new();
    let doc_uri = uri("cycle.md");

    let content = "# Overview\n\nContent #cycling ^block-cycle";

    realm.add_document(doc_uri.clone(), index_from(content));
    assert_eq!(realm.lookup_heading("overview").len(), 1);
    assert!(realm.lookup_block("block-cycle").is_some());

    realm.remove_document(&doc_uri);
    assert!(realm.lookup_heading("overview").is_empty());
    assert!(realm.lookup_block("block-cycle").is_none());

    // Re-add same content
    realm.add_document(doc_uri.clone(), index_from(content));
    assert_eq!(realm.lookup_heading("overview").len(), 1);
    assert!(realm.lookup_block("block-cycle").is_some());
    let has_tag = realm.tag_counts().iter().any(|(n, _)| n == "cycling");
    assert!(has_tag, "tag should be present after re-add");
}

#[test]
fn test_cross_doc_same_slug_remove_first() {
    // Two docs with same slug. Remove first. Verify only second remains.
    let mut realm = RealmIndex::new();

    let uri_a = uri("cross-a.md");
    let uri_b = uri("cross-b.md");
    realm.add_document(uri_a.clone(), index_from("# Overview\n\nDoc A"));
    realm.add_document(uri_b.clone(), index_from("# Overview\n\nDoc B"));

    assert_eq!(realm.lookup_heading("overview").len(), 2);

    realm.remove_document(&uri_a);

    let results = realm.lookup_heading("overview");
    assert_eq!(results.len(), 1, "only doc B's heading should remain");
    assert_eq!(results[0].0.as_str(), uri_b.as_str());
}

#[test]
fn test_lookup_heading_returns_correct_strings() {
    // Verify returned ResolvedHeading has correct text and slug Strings
    // (not corrupted by interning).
    let mut realm = RealmIndex::new();
    let doc_uri = uri("strings.md");
    realm.add_document(doc_uri.clone(), index_from("# Hello World\n\nContent"));

    let results = realm.lookup_heading("hello-world");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.text, "Hello World");
    assert_eq!(results[0].1.slug, "hello-world");
    assert_eq!(results[0].1.level, 1);
}

#[test]
fn test_tag_counts_after_interning() {
    // Two docs with overlapping tags. Verify counts are correct.
    let mut realm = RealmIndex::new();

    realm.add_document(uri("tag-a.md"), index_from("Content #alpha #beta here"));
    realm.add_document(uri("tag-b.md"), index_from("More #beta #gamma content"));

    let counts = realm.tag_counts();
    let alpha = counts.iter().find(|(n, _)| n == "alpha");
    let beta = counts.iter().find(|(n, _)| n == "beta");
    let gamma = counts.iter().find(|(n, _)| n == "gamma");

    assert_eq!(alpha.unwrap().1, 1, "alpha in 1 doc");
    assert_eq!(beta.unwrap().1, 2, "beta in 2 docs");
    assert_eq!(gamma.unwrap().1, 1, "gamma in 1 doc");
}

#[test]
fn test_block_lookup_returns_correct_id() {
    let mut realm = RealmIndex::new();
    realm.add_document(uri("block-id.md"), index_from("Paragraph ^my-block"));

    let result = realm.lookup_block("my-block");
    assert!(result.is_some());
    let (_uri, block) = result.unwrap();
    assert_eq!(block.id, "my-block");
}

#[test]
fn test_remove_document_clears_cross_doc_maps() {
    // After removing the only doc, all cross-doc maps should be empty.
    let mut realm = RealmIndex::new();
    let doc_uri = uri("solo.md");
    realm.add_document(
        doc_uri.clone(),
        index_from("# Heading\n\nContent #tag ^block-id"),
    );

    realm.remove_document(&doc_uri);

    assert!(realm.lookup_heading("heading").is_empty());
    assert!(realm.lookup_block("block-id").is_none());
    assert!(realm.tag_counts().is_empty());
}
