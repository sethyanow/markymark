use std::path::PathBuf;

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::DocumentUri;
use markymark_index::resolution;
use markymark_index::{DocumentIndex, RealmIndex, ResolvedTarget, StructuredDocumentIndex};

/// Helper: build a DocumentIndex from raw text via the engine path.
fn index_from(source: &str) -> DocumentIndex {
    DocumentIndex::from_text(source)
}

/// Helper: create a file:// URI from a filename.
fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{}", name)))
}

/// Build a test realm with multiple documents for resolution tests.
async fn test_realm() -> (RealmIndex, DocumentUri, DocumentUri, DocumentUri) {
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
    realm.add_document(uri_a.clone(), idx_a).await;

    // Document B: page-b.md
    let uri_b = uri("page-b.md");
    let idx_b = index_from(
        "# Page B Title\n\n\
         Content referencing [[page-a#section-one]].\n\n\
         ## Overview\n\n\
         Details ^block-b1\n\n\
         ## Summary",
    );
    realm.add_document(uri_b.clone(), idx_b).await;

    // Document C: notes/daily.md
    let uri_c = uri("notes/daily.md");
    let idx_c = index_from(
        "# Daily Notes\n\n\
         Check [[page-a]] and [[page-b#overview]].\n\n\
         ## Tasks\n\n\
         - [ ] Something ^task-block",
    );
    realm.add_document(uri_c.clone(), idx_c).await;

    (realm, uri_a, uri_b, uri_c)
}

// ---------------------------------------------------------------------------
// Wiki link resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_wiki_link_to_page() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm().await;

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

#[tokio::test]
async fn test_resolve_wiki_link_to_heading() {
    let (realm, uri_a, _uri_b, uri_c) = test_realm().await;

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

#[tokio::test]
async fn test_resolve_wiki_link_current_page_heading() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

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

#[tokio::test]
async fn test_resolve_markdown_link_to_heading() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

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

/// Build a test realm with documents in subdirectories for path-resolution tests.
///
/// Layout:
///   /vault/docs/api/endpoints.md
///   /vault/docs/guide/overview.md
///   /vault/docs/api/auth.md   (same directory as endpoints.md)
///   /vault/index.md
async fn path_realm() -> (
    RealmIndex,
    DocumentUri,
    DocumentUri,
    DocumentUri,
    DocumentUri,
) {
    let mut realm = RealmIndex::new();

    // /vault/docs/api/endpoints.md
    let uri_endpoints =
        DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/docs/api/endpoints.md"));
    let idx_endpoints = index_from("# API Endpoints\n\n## List Endpoints\n\nSome content.");
    realm
        .add_document(uri_endpoints.clone(), idx_endpoints)
        .await;

    // /vault/docs/guide/overview.md — same stem as... nothing yet, but in a different dir
    let uri_overview =
        DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/docs/guide/overview.md"));
    let idx_overview = index_from("# Guide Overview\n\n## Introduction\n\nGuide content.");
    realm.add_document(uri_overview.clone(), idx_overview).await;

    // /vault/docs/api/auth.md — same directory as endpoints.md
    let uri_auth =
        DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/docs/api/auth.md"));
    let idx_auth = index_from("# Auth\n\n## OAuth Flow\n\nAuth content.");
    realm.add_document(uri_auth.clone(), idx_auth).await;

    // /vault/index.md — root level doc
    let uri_index = DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/index.md"));
    let idx_index = index_from("# Index\n\nRoot document.");
    realm.add_document(uri_index.clone(), idx_index).await;

    (realm, uri_endpoints, uri_overview, uri_auth, uri_index)
}

// ---------------------------------------------------------------------------
// Markdown link → document resolution (stem-only)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_markdown_link_to_document_by_stem() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

    // [text](page-b.md) from page A — stem "page-b" should resolve to page-b.md
    let result = resolution::resolve_markdown_link(&realm, &uri_a, "page-b.md", None);
    assert!(
        result.is_some(),
        "[text](page-b.md) should resolve to the document"
    );

    match result.unwrap() {
        ResolvedTarget::Document(resolved_uri) => {
            assert_eq!(resolved_uri.as_str(), uri("page-b.md").as_str());
        }
        other => panic!("expected Document variant, got {:?}", other),
    }
}

#[tokio::test]
async fn test_resolve_markdown_link_to_document_with_anchor() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

    // [text](page-b.md#overview) from page A should resolve to the heading in page-b.md
    let result = resolution::resolve_markdown_link(&realm, &uri_a, "page-b.md", Some("overview"));
    assert!(
        result.is_some(),
        "[text](page-b.md#overview) should resolve to heading"
    );

    match result.unwrap() {
        ResolvedTarget::Heading {
            uri: resolved_uri,
            slug,
            ..
        } => {
            assert_eq!(resolved_uri.as_str(), uri("page-b.md").as_str());
            assert_eq!(slug, "overview");
        }
        other => panic!("expected Heading variant, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Markdown link → path-relative resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_markdown_link_path_relative_in_same_dir() {
    let (realm, uri_endpoints, _uri_overview, uri_auth, _uri_index) = path_realm().await;

    // From /vault/docs/api/endpoints.md → [text](auth.md)
    // "auth.md" has no directory segment, so stem-only resolution applies.
    // Stem "auth" resolves to /vault/docs/api/auth.md.
    let result = resolution::resolve_markdown_link(&realm, &uri_endpoints, "auth.md", None);
    assert!(
        result.is_some(),
        "[text](auth.md) should resolve via stem to auth.md"
    );
    match result.unwrap() {
        ResolvedTarget::Document(resolved) => {
            assert_eq!(resolved.as_str(), uri_auth.as_str());
        }
        other => panic!("expected Document, got {:?}", other),
    }
}

#[tokio::test]
async fn test_resolve_markdown_link_path_relative_with_directory_segment() {
    let (realm, uri_endpoints, _uri_overview, _uri_auth, _uri_index) = path_realm().await;

    // From /vault/docs/api/endpoints.md → [text](../guide/overview.md)
    // Has directory segment → try path-relative: resolves to /vault/docs/guide/overview.md.
    let result =
        resolution::resolve_markdown_link(&realm, &uri_endpoints, "../guide/overview.md", None);
    assert!(
        result.is_some(),
        "[text](../guide/overview.md) should resolve path-relatively"
    );
    match result.unwrap() {
        ResolvedTarget::Document(resolved) => {
            let expected = DocumentUri::from_file_path(&std::path::PathBuf::from(
                "/vault/docs/guide/overview.md",
            ));
            assert_eq!(resolved.as_str(), expected.as_str());
        }
        other => panic!("expected Document, got {:?}", other),
    }
}

#[tokio::test]
async fn test_resolve_markdown_link_path_relative_with_anchor() {
    let (realm, uri_endpoints, _uri_overview, _uri_auth, _uri_index) = path_realm().await;

    // From /vault/docs/api/endpoints.md → [text](../guide/overview.md#introduction)
    let result = resolution::resolve_markdown_link(
        &realm,
        &uri_endpoints,
        "../guide/overview.md",
        Some("introduction"),
    );
    assert!(
        result.is_some(),
        "[text](../guide/overview.md#introduction) should resolve to heading"
    );
    match result.unwrap() {
        ResolvedTarget::Heading {
            uri: resolved_uri,
            slug,
            ..
        } => {
            let expected = DocumentUri::from_file_path(&std::path::PathBuf::from(
                "/vault/docs/guide/overview.md",
            ));
            assert_eq!(resolved_uri.as_str(), expected.as_str());
            assert_eq!(slug, "introduction");
        }
        other => panic!("expected Heading, got {:?}", other),
    }
}

#[tokio::test]
async fn test_resolve_markdown_link_path_relative_falls_back_to_stem() {
    let (realm, _uri_endpoints, _uri_overview, uri_auth, _uri_index) = path_realm().await;

    // From /vault/index.md → [text](api/auth.md)
    // Path-relative: /vault/api/auth.md — does NOT exist.
    // Fall back to stem "auth" → finds /vault/docs/api/auth.md.
    let uri_root = DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/index.md"));
    let result = resolution::resolve_markdown_link(&realm, &uri_root, "api/auth.md", None);
    assert!(
        result.is_some(),
        "[text](api/auth.md) should fall back to stem resolution"
    );
    match result.unwrap() {
        ResolvedTarget::Document(resolved) => {
            assert_eq!(resolved.as_str(), uri_auth.as_str());
        }
        other => panic!("expected Document, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// External URL filtering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_markdown_link_https_url_returns_none() {
    // Realm has a document whose stem happens to match the hostname of an external URL.
    // resolve_markdown_link must NOT return it as a false-positive match.
    let mut realm = RealmIndex::new();
    let local_uri = DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/example.com.md"));
    let idx = index_from("# Example\n\nSome content.");
    realm.add_document(local_uri, idx).await;

    let from = DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/page.md"));
    let result = resolution::resolve_markdown_link(&realm, &from, "https://example.com", None);
    assert!(
        result.is_none(),
        "external https:// URL must not resolve to a local document"
    );
}

#[test]
fn test_resolve_markdown_link_mailto_url_returns_none() {
    let realm = RealmIndex::new();
    let from = DocumentUri::from_file_path(&std::path::PathBuf::from("/vault/page.md"));
    let result = resolution::resolve_markdown_link(&realm, &from, "mailto:user@example.com", None);
    assert!(
        result.is_none(),
        "mailto: URL must not attempt local resolution"
    );
}

// ---------------------------------------------------------------------------
// Block reference resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_block_ref() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

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

#[tokio::test]
async fn test_unresolved_wiki_link() {
    let (realm, uri_a, _uri_b, _uri_c) = test_realm().await;

    // [[nonexistent-page]] should not resolve
    let result = resolution::resolve_wiki_link(&realm, &uri_a, "nonexistent-page", None);
    assert!(result.is_none(), "[[nonexistent-page]] should return None");
}

// ---------------------------------------------------------------------------
// Cross-document resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_across_documents() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm().await;

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

#[tokio::test]
async fn test_resolve_case_insensitive_page() {
    let (realm, _uri_a, uri_b, uri_c) = test_realm().await;

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
