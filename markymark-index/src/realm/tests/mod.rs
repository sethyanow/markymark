mod core;
mod helpers;

use super::*;
use helpers::{
    code_span, make_md_index, make_md_index_with_code_spans, make_structured_index, test_key, uri,
};
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
        ],
    );
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

// ── Layer 3: Incremental cross-doc index updates (marky-c9dm) ──

#[tokio::test]
async fn test_contribution_built_on_add() {
    let mut realm = RealmIndex::new();
    let u = uri("contrib_add.md");
    realm
        .add_document(
            u.clone(),
            make_md_index("# Intro\n\n#tag1 #tag2\n\n^block1"),
        )
        .await;

    let key = u.as_str().to_string();
    let contrib = realm
        .contributions
        .get(&key)
        .expect("contribution should be stored on add");

    // Verify heading slugs
    assert!(
        contrib
            .heading_slugs
            .iter()
            .any(|s| realm.interner.resolve(s) == "intro"),
        "contribution should contain heading slug 'intro'"
    );

    // Verify tag names
    assert!(
        contrib
            .tag_names
            .iter()
            .any(|s| realm.interner.resolve(s) == "tag1"),
        "contribution should contain tag 'tag1'"
    );
    assert!(
        contrib
            .tag_names
            .iter()
            .any(|s| realm.interner.resolve(s) == "tag2"),
        "contribution should contain tag 'tag2'"
    );

    // Verify block ids
    assert!(
        contrib
            .block_ids
            .iter()
            .any(|s| realm.interner.resolve(s) == "block1"),
        "contribution should contain block id 'block1'"
    );
}

#[tokio::test]
async fn test_contribution_removed_on_remove() {
    let mut realm = RealmIndex::new();
    let u = uri("contrib_remove.md");
    realm
        .add_document(u.clone(), make_md_index("# Heading\n\n#mytag"))
        .await;

    let key = u.as_str().to_string();
    assert!(
        realm.contributions.contains_key(&key),
        "contribution present after add"
    );

    realm.remove_document(&u).await;
    assert!(
        !realm.contributions.contains_key(&key),
        "contribution removed after remove"
    );
}

#[tokio::test]
async fn test_update_no_structural_change() {
    let mut realm = RealmIndex::new();
    let u = uri("update_noop.md");
    // Add doc with 3 headings and 2 tags
    realm
        .add_document(
            u.clone(),
            make_md_index("# Intro\n\n## Details\n\n### Deep\n\n#rust #zig"),
        )
        .await;

    let heading_count_before = realm
        .slug_to_headings
        .values()
        .map(|v| v.len())
        .sum::<usize>();
    let tag_count_before: usize = realm.tag_counts().iter().map(|(_, c)| c).sum();

    // Update with identical structure but different content (simulating range shift)
    realm
        .update_document(
            u.clone(),
            make_md_index("# Intro\n\n## Details\n\n### Deep\n\n#rust #zig"),
        )
        .await;

    let heading_count_after = realm
        .slug_to_headings
        .values()
        .map(|v| v.len())
        .sum::<usize>();
    let tag_count_after: usize = realm.tag_counts().iter().map(|(_, c)| c).sum();

    assert_eq!(
        heading_count_before, heading_count_after,
        "heading entries unchanged on no-op update"
    );
    assert_eq!(
        tag_count_before, tag_count_after,
        "tag entries unchanged on no-op update"
    );
    assert!(
        realm.docs.contains_key(u.as_str()),
        "document still in docs"
    );
}

#[tokio::test]
async fn test_update_heading_added() {
    let mut realm = RealmIndex::new();
    let u = uri("update_heading_add.md");
    realm
        .add_document(u.clone(), make_md_index("# Intro"))
        .await;

    // Update: add a second heading
    realm
        .update_document(u.clone(), make_md_index("# Intro\n\n## Details"))
        .await;

    let intro_spur = realm.interner.get("intro").expect("intro interned");
    let details_spur = realm.interner.get("details").expect("details interned");

    let intro_entries = realm
        .slug_to_headings
        .get(&intro_spur)
        .expect("intro present");
    assert!(
        intro_entries.iter().any(|(uri, _)| uri == &u),
        "intro still has our doc"
    );

    let details_entries = realm
        .slug_to_headings
        .get(&details_spur)
        .expect("details present");
    assert!(
        details_entries.iter().any(|(uri, _)| uri == &u),
        "details added for our doc"
    );
}

#[tokio::test]
async fn test_update_heading_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_heading_rm.md");
    realm
        .add_document(u.clone(), make_md_index("# Intro\n\n## Details"))
        .await;

    // Update: remove the second heading
    realm
        .update_document(u.clone(), make_md_index("# Intro"))
        .await;

    let intro_spur = realm.interner.get("intro").expect("intro interned");
    let details_spur = realm.interner.get("details").expect("details interned");

    assert!(
        realm.slug_to_headings.contains_key(&intro_spur),
        "intro still present"
    );

    let details_entries = realm.slug_to_headings.get(&details_spur);
    let has_our_doc = details_entries
        .map(|entries| entries.iter().any(|(uri, _)| uri == &u))
        .unwrap_or(false);
    assert!(!has_our_doc, "details removed for our doc");
}

#[tokio::test]
async fn test_update_tag_added_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_tags.md");
    realm
        .add_document(u.clone(), make_md_index("# H\n\n#rust #zig"))
        .await;

    // Update: remove zig, add wasm
    realm
        .update_document(u.clone(), make_md_index("# H\n\n#rust #wasm"))
        .await;

    // Use tag_counts() public API — tag_to_docs is lazily maintained.
    let counts: HashMap<String, usize> = realm.tag_counts().into_iter().collect();
    assert_eq!(counts.get("rust"), Some(&1), "rust tag still present");
    assert_eq!(counts.get("wasm"), Some(&1), "wasm tag added");
    assert_eq!(counts.get("zig"), None, "zig tag removed");
}

#[tokio::test]
async fn test_update_code_span_added() {
    let mut realm = RealmIndex::new();
    let u = uri("update_cs.md");
    realm.add_document(u.clone(), make_md_index("# H")).await;

    // Update: add a code span
    realm
        .update_document(
            u.clone(),
            make_md_index_with_code_spans(vec![code_span("HashMap")]),
        )
        .await;

    let hm_spur = realm.interner.get("HashMap").expect("HashMap interned");
    let entries = realm
        .code_span_to_docs
        .get(&hm_spur)
        .expect("HashMap entry exists");
    assert!(
        entries.iter().any(|(uri, _)| uri == &u),
        "code span added for our doc"
    );
}

#[tokio::test]
async fn test_update_block_id_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_block.md");
    realm
        .add_document(u.clone(), make_md_index("# H\n\ntext ^abc"))
        .await;

    // Update: remove block id
    realm
        .update_document(u.clone(), make_md_index("# H\n\ntext"))
        .await;

    let abc_spur = realm.interner.get("abc").expect("abc interned");
    let has_our_doc = realm
        .block_to_location
        .get(&abc_spur)
        .map(|entries| entries.iter().any(|(uri, _)| uri == &u))
        .unwrap_or(false);
    assert!(!has_our_doc, "block id removed for our doc");
}

#[tokio::test]
async fn test_update_preserves_other_docs_entries() {
    let mut realm = RealmIndex::new();
    let ua = uri("update_a.md");
    let ub = uri("update_b.md");
    // Both docs have heading "intro"
    realm
        .add_document(ua.clone(), make_md_index("# Intro"))
        .await;
    realm
        .add_document(ub.clone(), make_md_index("# Intro"))
        .await;

    let intro_spur = realm.interner.get("intro").expect("intro interned");
    let before = realm.slug_to_headings.get(&intro_spur).unwrap().len();
    assert_eq!(before, 2, "both docs contribute to intro");

    // Update doc A to remove "intro"
    realm
        .update_document(ua.clone(), make_md_index("# Changed"))
        .await;

    let intro_entries = realm.slug_to_headings.get(&intro_spur);
    let b_still_present = intro_entries
        .map(|entries| entries.iter().any(|(uri, _)| uri == &ub))
        .unwrap_or(false);
    assert!(
        b_still_present,
        "doc B's intro entry preserved after doc A update"
    );

    let a_removed = intro_entries
        .map(|entries| entries.iter().any(|(uri, _)| uri == &ua))
        .unwrap_or(false);
    assert!(!a_removed, "doc A's intro entry removed");
}

#[tokio::test]
async fn test_interner_memory_bounded_at_scale() {
    // Verify interner doesn't grow unboundedly when populating a vault.
    // Each doc contributes ~14 unique strings (10 heading slugs + 3 tags + 1 block ID).
    // With 200 docs: ~2800 unique slugs + ~23 shared tags + 200 block IDs ≈ ~3000 unique strings.
    // Threshold: 15K unique strings for 1000 docs (generous upper bound).
    let mut realm = RealmIndex::new();
    let n_docs = 200; // Use 200 for test speed (1000 in benchmarks)

    for i in 0..n_docs {
        let u = uri(&format!("vault_doc_{i}.md"));
        // Each doc: 10 headings, 3 tags (some shared), 1 block ID
        let source = format!(
            "# Doc {i} heading 0\n\n## Doc {i} heading 1\n\n### Doc {i} heading 2\n\n\
             # Doc {i} heading 3\n\n## Doc {i} heading 4\n\n### Doc {i} heading 5\n\n\
             # Doc {i} heading 6\n\n## Doc {i} heading 7\n\n### Doc {i} heading 8\n\n\
             # Doc {i} heading 9\n\n\
             text ^block-{i}\n\n\
             #project #status-{s} #topic-{t}\n",
            i = i,
            s = i % 3,
            t = i % 20
        );
        let index = make_md_index(&source);
        realm.add_document(u, index).await;
    }

    let interned = realm.interner_len();
    // 200 docs × ~14 unique strings ≈ ~2800, plus shared tags ≈ ~23
    // Allow generous headroom: 200 × 20 = 4000
    assert!(
        interned <= 4000,
        "interner grew beyond expected bound for {n_docs} docs: {interned} strings (expected <= 4000)"
    );
    // Sanity: should have at least n_docs × 2 entries (headings + stems)
    assert!(
        interned >= n_docs,
        "interner suspiciously small: {interned} for {n_docs} docs"
    );
}

// ── Layer 4: Lazy cold index tests ──

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
