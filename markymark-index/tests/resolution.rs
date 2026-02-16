use std::path::PathBuf;

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::DocumentUri;
use markymark_index::resolution;
use markymark_index::{DocumentIndex, RealmIndex, ResolvedTarget, StructuredDocumentIndex};
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

/// Build a test realm with multiple documents for resolution tests.
fn test_realm() -> (RealmIndex, DocumentUri, DocumentUri, DocumentUri) {
    let mut realm = RealmIndex::new();

    // Document A: page-a.md
    let uri_a = uri("page-a.md");
    let idx_a = index_from(
        "# Page A Title\n\n\
         Some content with [[page-b]] link.\n\n\
         ## Section One\n\n\
         Text here ^block-a1\n\n\
         ## Section Two\n\n\
         More text",
    );
    realm.add_document(uri_a.clone(), idx_a);

    // Document B: page-b.md
    let uri_b = uri("page-b.md");
    let idx_b = index_from(
        "# Page B Title\n\n\
         Content referencing [[page-a#section-one]].\n\n\
         ## Overview\n\n\
         Details ^block-b1\n\n\
         ## Summary",
    );
    realm.add_document(uri_b.clone(), idx_b);

    // Document C: notes/daily.md
    let uri_c = uri("notes/daily.md");
    let idx_c = index_from(
        "# Daily Notes\n\n\
         Check [[page-a]] and [[page-b#overview]].\n\n\
         ## Tasks\n\n\
         - [ ] Something ^task-block",
    );
    realm.add_document(uri_c.clone(), idx_c);

    (realm, uri_a, uri_b, uri_c)
}

// ---------------------------------------------------------------------------
// Wiki link resolution
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_wiki_link_to_page() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm();

    // [[page-b]] from document C should resolve to document B
    let result = resolution::resolve_wiki_link(&realm, &uri_c, "page-b", None);
    assert!(result.is_some(), "[[page-b]] should resolve to a document");

    match result.unwrap() {
        ResolvedTarget::Document(resolved_uri) => {
            assert_eq!(
                resolved_uri.as_str(),
                uri_b.as_str(),
                "should resolve to page-b.md"
            );
        }
        other => panic!("expected Document variant, got {:?}", other),
    }
}

#[test]
fn test_resolve_wiki_link_to_heading() {
    let (realm, uri_a, _uri_b, uri_c) = test_realm();

    // [[page-a#section-one]] from document C should resolve to heading in page A
    let result = resolution::resolve_wiki_link(&realm, &uri_c, "page-a", Some("section-one"));
    assert!(
        result.is_some(),
        "[[page-a#section-one]] should resolve to a heading"
    );

    match result.unwrap() {
        ResolvedTarget::Heading { uri, slug, text } => {
            assert_eq!(uri.as_str(), uri_a.as_str());
            assert_eq!(slug, "section-one");
            assert_eq!(text, "Section One");
        }
        other => panic!("expected Heading variant, got {:?}", other),
    }
}

#[test]
fn test_resolve_wiki_link_current_page_heading() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm();

    // [[#section-two]] from page A should resolve to heading in the same document
    let result = resolution::resolve_wiki_link(&realm, &uri_a, "", Some("section-two"));
    assert!(
        result.is_some(),
        "[[#section-two]] should resolve to heading in current doc"
    );

    match result.unwrap() {
        ResolvedTarget::Heading { uri, slug, text } => {
            assert_eq!(
                uri.as_str(),
                uri_a.as_str(),
                "should resolve within same doc"
            );
            assert_eq!(slug, "section-two");
            assert_eq!(text, "Section Two");
        }
        other => panic!("expected Heading variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Markdown link resolution
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_markdown_link_to_heading() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm();

    // [text](#section-one) from page A - same-page anchor link
    let result = resolution::resolve_markdown_link(&realm, &uri_a, "", Some("section-one"));
    assert!(
        result.is_some(),
        "[text](#section-one) should resolve to heading in current doc"
    );

    match result.unwrap() {
        ResolvedTarget::Heading { uri, slug, .. } => {
            assert_eq!(uri.as_str(), uri_a.as_str());
            assert_eq!(slug, "section-one");
        }
        other => panic!("expected Heading variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Block reference resolution
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_block_ref() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm();

    // ((block-a1)) should resolve to the block in page A
    let result = resolution::resolve_block_ref(&realm, "block-a1");
    assert!(result.is_some(), "((block-a1)) should resolve");

    match result.unwrap() {
        ResolvedTarget::Block { uri, id } => {
            assert_eq!(uri.as_str(), uri_a.as_str());
            assert_eq!(id, "block-a1");
        }
        other => panic!("expected Block variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Unresolved references
// ---------------------------------------------------------------------------

#[test]
fn test_unresolved_wiki_link() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm();

    // [[nonexistent-page]] should not resolve
    let result = resolution::resolve_wiki_link(&realm, &uri_a, "nonexistent-page", None);
    assert!(result.is_none(), "[[nonexistent-page]] should return None");
}

// ---------------------------------------------------------------------------
// Cross-document resolution
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_across_documents() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm();

    // From document C, resolve [[page-b#overview]] to heading in page B
    let result = resolution::resolve_wiki_link(&realm, &uri_c, "page-b", Some("overview"));
    assert!(result.is_some(), "cross-document wiki link should resolve");

    match result.unwrap() {
        ResolvedTarget::Heading { uri, slug, text } => {
            assert_eq!(uri.as_str(), uri_b.as_str());
            assert_eq!(slug, "overview");
            assert_eq!(text, "Overview");
        }
        other => panic!("expected Heading variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Case-insensitive page matching
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_case_insensitive_page() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm();

    // [[Page-B]] (different case) should still resolve to page-b.md
    let result = resolution::resolve_wiki_link(&realm, &uri_c, "Page-B", None);
    assert!(
        result.is_some(),
        "case-insensitive wiki link should resolve"
    );

    match result.unwrap() {
        ResolvedTarget::Document(resolved_uri) => {
            assert_eq!(resolved_uri.as_str(), uri_b.as_str());
        }
        other => panic!("expected Document variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Structured document key path resolution
// ---------------------------------------------------------------------------

fn make_key(path: &str, key: &str, depth: usize, vk: ValueKind) -> KeyEntry {
    KeyEntry {
        path: path.to_string(),
        key: key.to_string(),
        depth,
        value_kind: vk,
        key_range: markymark_core::Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 10),
        ),
        value_range: markymark_core::Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 10),
        ),
    }
}

#[test]
fn test_resolve_wiki_link_to_structured_key_path() {
    let mut realm = RealmIndex::new();

    let config_uri = uri("config.json");
    let st_index = StructuredDocumentIndex::from_ast(StructuredAst {
        source: String::new(),
        kind: DocumentKind::Json,
        keys: vec![
            make_key("database", "database", 0, ValueKind::Object),
            make_key("database.host", "host", 1, ValueKind::String),
            make_key("database.port", "port", 1, ValueKind::Number),
        ],
    });
    realm.add_structured_document(config_uri.clone(), st_index);

    let from_uri = uri("doc.md");

    // [[config#database.host]] should resolve to the key path
    let result = resolution::resolve_wiki_link(&realm, &from_uri, "config", Some("database.host"));
    assert!(
        result.is_some(),
        "[[config#database.host]] should resolve to a key path"
    );

    match result.unwrap() {
        ResolvedTarget::KeyPath {
            uri,
            path,
            value_kind,
            ..
        } => {
            assert_eq!(uri.as_str(), config_uri.as_str());
            assert_eq!(path, "database.host");
            assert_eq!(value_kind, ValueKind::String);
        }
        other => panic!("expected KeyPath variant, got {:?}", other),
    }
}

#[test]
fn test_resolve_wiki_link_to_structured_doc_without_fragment() {
    let mut realm = RealmIndex::new();

    let config_uri = uri("config.json");
    let st_index = StructuredDocumentIndex::from_ast(StructuredAst {
        source: String::new(),
        kind: DocumentKind::Json,
        keys: vec![make_key("name", "name", 0, ValueKind::String)],
    });
    realm.add_structured_document(config_uri.clone(), st_index);

    let from_uri = uri("doc.md");

    // [[config]] should resolve to the document
    let result = resolution::resolve_wiki_link(&realm, &from_uri, "config", None);
    assert!(result.is_some(), "[[config]] should resolve to a document");

    match result.unwrap() {
        ResolvedTarget::Document(resolved_uri) => {
            assert_eq!(resolved_uri.as_str(), config_uri.as_str());
        }
        other => panic!("expected Document variant, got {:?}", other),
    }
}

#[test]
fn test_resolve_wiki_link_nonexistent_key_path() {
    let mut realm = RealmIndex::new();

    let config_uri = uri("config.yaml");
    let st_index = StructuredDocumentIndex::from_ast(StructuredAst {
        source: String::new(),
        kind: DocumentKind::Yaml,
        keys: vec![make_key("server", "server", 0, ValueKind::Object)],
    });
    realm.add_structured_document(config_uri, st_index);

    let from_uri = uri("doc.md");

    // [[config#nonexistent.path]] should not resolve
    let result =
        resolution::resolve_wiki_link(&realm, &from_uri, "config", Some("nonexistent.path"));
    assert!(result.is_none(), "nonexistent key path should return None");
}
