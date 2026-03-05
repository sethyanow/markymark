//! Layer 4: Lazy cold index tests.
//!
//! Tests for deferred tag-to-docs rebuilding: multiple updates before query,
//! remove/add while dirty, and fast-path no-op detection.

use std::collections::HashMap;

use super::helpers::{make_md_index, uri};
use super::super::*;

#[tokio::test]
async fn test_lazy_tags_multiple_updates_before_query() {
    let mut realm = RealmIndex::new();
    let u = uri("lazy_tags.md");
    realm
        .add_document(u.clone(), make_md_index("# H\n\n#rust #zig"))
        .await;

    // Rapid sequence of structural edits changing tags — no query between them.
    realm
        .update_document(u.clone(), make_md_index("# H\n\n#rust #wasm"))
        .await;
    realm
        .update_document(u.clone(), make_md_index("# H\n\n#go #wasm"))
        .await;
    realm
        .update_document(u.clone(), make_md_index("# H\n\n#go #python #wasm"))
        .await;

    // Single query should reflect final state.
    let counts: HashMap<String, usize> = realm.tag_counts().into_iter().collect();
    assert_eq!(counts.get("go"), Some(&1));
    assert_eq!(counts.get("python"), Some(&1));
    assert_eq!(counts.get("wasm"), Some(&1));
    assert_eq!(counts.get("rust"), None, "rust removed in second update");
    assert_eq!(counts.get("zig"), None, "zig removed in first update");
}

#[tokio::test]
async fn test_lazy_tags_remove_after_dirty_update() {
    let mut realm = RealmIndex::new();
    let u1 = uri("doc1.md");
    let u2 = uri("doc2.md");
    realm
        .add_document(u1.clone(), make_md_index("# A\n\n#shared #unique1"))
        .await;
    realm
        .add_document(u2.clone(), make_md_index("# B\n\n#shared #unique2"))
        .await;

    // Update doc1 tags (makes tag index dirty)
    realm
        .update_document(u1.clone(), make_md_index("# A\n\n#shared #replaced1"))
        .await;

    // Remove doc2 — must clean tag index first to avoid stale entries.
    realm.remove_document(&u2).await;

    let counts: HashMap<String, usize> = realm.tag_counts().into_iter().collect();
    assert_eq!(counts.get("shared"), Some(&1), "only doc1 has shared now");
    assert_eq!(counts.get("replaced1"), Some(&1));
    assert_eq!(counts.get("unique1"), None, "unique1 removed by update");
    assert_eq!(counts.get("unique2"), None, "unique2 removed with doc2");
}

#[tokio::test]
async fn test_lazy_tags_add_after_dirty_update() {
    let mut realm = RealmIndex::new();
    let u1 = uri("existing.md");
    realm
        .add_document(u1.clone(), make_md_index("# A\n\n#alpha"))
        .await;

    // Update makes tags dirty
    realm
        .update_document(u1.clone(), make_md_index("# A\n\n#beta"))
        .await;

    // Add new document — must clean tag index first.
    let u2 = uri("new.md");
    realm
        .add_document(u2.clone(), make_md_index("# B\n\n#gamma #beta"))
        .await;

    let counts: HashMap<String, usize> = realm.tag_counts().into_iter().collect();
    assert_eq!(counts.get("alpha"), None, "alpha removed by update");
    assert_eq!(counts.get("beta"), Some(&2), "beta in both docs");
    assert_eq!(counts.get("gamma"), Some(&1), "gamma in new doc");
}

#[tokio::test]
async fn test_lazy_tags_fast_path_keeps_tags_clean() {
    let mut realm = RealmIndex::new();
    let u = uri("fastpath.md");
    realm
        .add_document(u.clone(), make_md_index("# H\n\n#rust"))
        .await;

    // Same structure — fast path, tags should NOT be dirtied.
    realm
        .update_document(u.clone(), make_md_index("# H\n\n#rust"))
        .await;

    // tag_to_docs should still be clean and correct.
    assert!(!realm.tags_dirty, "fast path should not dirty tags");
    let counts: HashMap<String, usize> = realm.tag_counts().into_iter().collect();
    assert_eq!(counts.get("rust"), Some(&1));
}
