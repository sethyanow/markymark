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
