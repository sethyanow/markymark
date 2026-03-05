//! Code span cross-doc index tests and stem index tests (Layer 2).

use super::helpers::{
    code_span, make_md_index, make_md_index_with_code_spans, make_structured_index, test_key, uri,
};
use super::super::*;
use markymark_core::structured::ValueKind;
use std::path::PathBuf;

// ── Code span cross-doc index tests ──────────────────────────────────────

#[tokio::test]
async fn test_add_document_populates_code_spans() {
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans(vec![code_span("HashMap")]);
    realm.add_document(u.clone(), index).await;

    let results = realm.lookup_code_span("HashMap");
    assert_eq!(results.len(), 1, "expected 1 code span entry");
    assert_eq!(results[0].0, u);
    assert_eq!(results[0].1.text, "HashMap");
}

#[tokio::test]
async fn test_remove_document_cleans_code_spans() {
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans(vec![code_span("Vec")]);
    realm.add_document(u.clone(), index).await;
    assert_eq!(realm.lookup_code_span("Vec").len(), 1);

    realm.remove_document(&u).await;
    assert!(
        realm.lookup_code_span("Vec").is_empty(),
        "code span should be cleaned up after removal"
    );
}

#[tokio::test]
async fn test_code_span_dedup_per_document() {
    // Same text 3x in one doc → only 1 entry per doc in cross-doc index.
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans(vec![
        code_span("Result"),
        code_span("Result"),
        code_span("Result"),
    ]);
    realm.add_document(u.clone(), index).await;

    let results = realm.lookup_code_span("Result");
    assert_eq!(
        results.len(),
        1,
        "same text 3x in one doc should produce 1 entry, got {}",
        results.len()
    );
}

#[tokio::test]
async fn test_code_span_cross_doc() {
    // Two docs with same code span text → both appear in lookup.
    let mut realm = RealmIndex::new();
    let u1 = uri("a.md");
    let u2 = uri("b.md");
    realm
        .add_document(
            u1.clone(),
            make_md_index_with_code_spans(vec![code_span("Option")]),
        )
        .await;
    realm
        .add_document(
            u2.clone(),
            make_md_index_with_code_spans(vec![code_span("Option")]),
        )
        .await;

    let results = realm.lookup_code_span("Option");
    assert_eq!(results.len(), 2, "expected 2 docs, got {}", results.len());
    let uris: Vec<_> = results.iter().map(|(u, _)| u.clone()).collect();
    assert!(uris.contains(&u1));
    assert!(uris.contains(&u2));
}

#[test]
fn test_lookup_code_span_not_found() {
    let realm = RealmIndex::new();
    assert!(
        realm.lookup_code_span("NonExistent").is_empty(),
        "empty realm should return no results"
    );
}

// ── Stem index tests (Layer 2: marky-e2nu) ───────────────────────────────

#[tokio::test]
async fn test_stem_index_basic_lookup() {
    let mut realm = RealmIndex::new();
    let u = uri("notes.md");
    realm
        .add_document(u.clone(), make_md_index("# Hello"))
        .await;

    let result = realm.find_uri_by_stem("notes");
    assert_eq!(
        result,
        Some(u),
        "basic stem lookup should find the document"
    );
}

#[tokio::test]
async fn test_stem_index_case_insensitive() {
    let mut realm = RealmIndex::new();
    let u = DocumentUri::from_file_path(&PathBuf::from("/vault/MyPage.md"));
    realm.add_document(u.clone(), make_md_index("# Page")).await;

    assert_eq!(
        realm.find_uri_by_stem("mypage"),
        Some(u.clone()),
        "lowercase query should match mixed-case stem"
    );
    assert_eq!(
        realm.find_uri_by_stem("MYPAGE"),
        Some(u),
        "uppercase query should match mixed-case stem"
    );
}

#[tokio::test]
async fn test_stem_index_cross_doc_same_stem() {
    let mut realm = RealmIndex::new();
    let u1 = DocumentUri::from_file_path(&PathBuf::from("/vault/a/readme.md"));
    let u2 = DocumentUri::from_file_path(&PathBuf::from("/vault/b/readme.md"));
    realm.add_document(u1.clone(), make_md_index("# A")).await;
    realm.add_document(u2.clone(), make_md_index("# B")).await;

    let result = realm.find_uri_by_stem("readme");
    assert_eq!(
        result,
        Some(u1),
        "same-stem collision should return first-added document"
    );
}

#[tokio::test]
async fn test_stem_index_remove_then_lookup() {
    let mut realm = RealmIndex::new();
    let u = uri("page.md");
    realm.add_document(u.clone(), make_md_index("# Page")).await;
    assert!(realm.find_uri_by_stem("page").is_some());

    realm.remove_document(&u).await;
    assert_eq!(
        realm.find_uri_by_stem("page"),
        None,
        "stem lookup should return None after document removal"
    );
}

#[tokio::test]
async fn test_stem_index_replace_document() {
    let mut realm = RealmIndex::new();
    let u = uri("page.md");
    realm.add_document(u.clone(), make_md_index("# Old")).await;
    realm.add_document(u.clone(), make_md_index("# New")).await;

    let result = realm.find_uri_by_stem("page");
    assert_eq!(
        result,
        Some(u),
        "stem lookup should work after document replacement (no duplicates)"
    );
}

#[test]
fn test_stem_index_structured_document() {
    let mut realm = RealmIndex::new();
    let u = uri("settings.json");
    realm.add_structured_document(
        u.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("theme", "theme", 0, ValueKind::String)],
        ),
    );

    let result = realm.find_uri_by_stem("settings");
    assert_eq!(
        result,
        Some(u),
        "stem lookup should find structured documents too"
    );
}

#[tokio::test]
async fn test_stem_index_remove_one_of_two_same_stem() {
    let mut realm = RealmIndex::new();
    let u1 = DocumentUri::from_file_path(&PathBuf::from("/vault/a/readme.md"));
    let u2 = DocumentUri::from_file_path(&PathBuf::from("/vault/b/readme.md"));
    realm.add_document(u1.clone(), make_md_index("# A")).await;
    realm.add_document(u2.clone(), make_md_index("# B")).await;

    realm.remove_document(&u1).await;

    let result = realm.find_uri_by_stem("readme");
    assert_eq!(
        result,
        Some(u2),
        "after removing first doc, stem should resolve to remaining doc"
    );
}
