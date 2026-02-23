use super::helpers::{detect_journal_date, resolve_relative_path};
use super::*;
use crate::document::{CodeSpanOwned, DocumentIndex};
use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst};
use std::path::PathBuf;

fn make_md_index(source: &str) -> DocumentIndex {
    let ast = markymark_parser::parse(source).unwrap();
    DocumentIndex::from_ast(ast)
}

/// Build a markdown index whose code_spans contain the given identifiers.
///
/// Constructs a source string with backtick code spans so `from_ast`
/// (which delegates to from_scan) extracts them naturally.
fn make_md_index_with_code_spans(
    _base_source: &str,
    code_spans: Vec<CodeSpanOwned>,
) -> DocumentIndex {
    // Build source text: heading + one backtick code span per entry
    let mut source = String::from("# Intro\n\n");
    for cs in &code_spans {
        source.push('`');
        source.push_str(&cs.text);
        source.push_str("` ");
    }
    source.push('\n');
    let ast = markymark_parser::parse(&source).unwrap();
    DocumentIndex::from_ast(ast)
}

fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}")))
}

fn make_structured_index(kind: DocumentKind, keys: Vec<KeyEntry>) -> StructuredDocumentIndex {
    let ast = StructuredAst {
        source: String::new(),
        kind,
        keys,
    };
    StructuredDocumentIndex::from_ast(ast)
}

fn test_key(path: &str, key_name: &str, depth: usize, vk: ValueKind) -> KeyEntry {
    KeyEntry {
        path: path.to_string(),
        key: key_name.to_string(),
        depth,
        value_kind: vk,
        key_range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
        value_range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
    }
}

#[test]
fn test_add_markdown_document() {
    let mut realm = RealmIndex::new();
    let uri = uri("test.md");
    let index = make_md_index("# Hello\n## World");
    realm.add_document(uri.clone(), index);

    assert_eq!(realm.document_count(), 1);
    assert_eq!(realm.markdown_count(), 1);
    assert_eq!(realm.structured_count(), 0);
    assert!(realm.get_document(&uri).is_some());
}

#[test]
fn test_add_structured_document() {
    let mut realm = RealmIndex::new();
    let uri = uri("config.json");
    let index = make_structured_index(
        DocumentKind::Json,
        vec![
            test_key("db", "db", 0, ValueKind::Object),
            test_key("db.host", "host", 1, ValueKind::String),
        ],
    );
    realm.add_structured_document(uri.clone(), index);

    assert_eq!(realm.document_count(), 1);
    assert_eq!(realm.markdown_count(), 0);
    assert_eq!(realm.structured_count(), 1);
    assert!(realm.get_structured_document(&uri).is_some());
    assert!(realm.get_document(&uri).is_none()); // Not markdown
}

#[test]
fn test_mixed_documents() {
    let mut realm = RealmIndex::new();

    let md_uri = uri("doc.md");
    realm.add_document(md_uri.clone(), make_md_index("# Title"));

    let json_uri = uri("config.json");
    realm.add_structured_document(
        json_uri.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("name", "name", 0, ValueKind::String)],
        ),
    );

    assert_eq!(realm.document_count(), 2);
    assert_eq!(realm.markdown_count(), 1);
    assert_eq!(realm.structured_count(), 1);

    // iter_documents only returns markdown
    assert_eq!(realm.iter_documents().count(), 1);
    // iter_all_documents returns everything
    assert_eq!(realm.iter_all_documents().count(), 2);
}

#[test]
fn test_remove_structured_document() {
    let mut realm = RealmIndex::new();
    let uri = uri("config.json");
    realm.add_structured_document(
        uri.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("name", "name", 0, ValueKind::String)],
        ),
    );

    assert_eq!(realm.structured_count(), 1);
    realm.remove_document(&uri);
    assert_eq!(realm.structured_count(), 0);
}

#[test]
fn test_search_key_paths() {
    let mut realm = RealmIndex::new();
    let uri = uri("config.yaml");
    realm.add_structured_document(
        uri,
        make_structured_index(
            DocumentKind::Yaml,
            vec![
                test_key("database", "database", 0, ValueKind::Object),
                test_key("database.host", "host", 1, ValueKind::String),
                test_key("logging", "logging", 0, ValueKind::Object),
            ],
        ),
    );

    let results = realm.search_key_paths("host");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "database.host");

    let results = realm.search_key_paths("database");
    assert_eq!(results.len(), 2); // "database" + "database.host"
}

#[test]
fn test_key_path_count() {
    let mut realm = RealmIndex::new();
    realm.add_structured_document(
        uri("a.json"),
        make_structured_index(
            DocumentKind::Json,
            vec![
                test_key("x", "x", 0, ValueKind::String),
                test_key("y", "y", 0, ValueKind::String),
            ],
        ),
    );
    realm.add_structured_document(
        uri("b.toml"),
        make_structured_index(
            DocumentKind::Toml,
            vec![test_key("z", "z", 0, ValueKind::String)],
        ),
    );

    assert_eq!(realm.key_path_count(), 3);
}

#[test]
fn test_markdown_cross_doc_still_works() {
    let mut realm = RealmIndex::new();
    let uri = uri("doc.md");
    realm.add_document(uri.clone(), make_md_index("# Hello\n## World"));

    // Heading lookup should still work
    let headings = realm.lookup_heading("hello");
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].0, uri);
}

#[test]
fn test_replace_structured_document() {
    let mut realm = RealmIndex::new();
    let uri = uri("config.json");

    realm.add_structured_document(
        uri.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("old", "old", 0, ValueKind::String)],
        ),
    );

    // Replace with new content
    realm.add_structured_document(
        uri.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("new", "new", 0, ValueKind::String)],
        ),
    );

    assert_eq!(realm.document_count(), 1);
    let idx = realm.get_structured_document(&uri).unwrap();
    assert_eq!(idx.keys()[0].key, "new");
}

#[test]
fn test_get_any_document() {
    let mut realm = RealmIndex::new();

    let md_uri = uri("doc.md");
    realm.add_document(md_uri.clone(), make_md_index("# Title"));

    let json_uri = uri("config.json");
    realm.add_structured_document(
        json_uri.clone(),
        make_structured_index(
            DocumentKind::Json,
            vec![test_key("k", "k", 0, ValueKind::String)],
        ),
    );

    let md_any = realm.get_any_document(&md_uri).unwrap();
    assert!(md_any.is_markdown());

    let json_any = realm.get_any_document(&json_uri).unwrap();
    assert!(json_any.is_structured());
}

// --- Journal page detection tests (marky-waw) ---

#[test]
fn test_journal_date_detected_underscore_separator() {
    // Bug this catches: function returns None when it should match YYYY_MM_DD format
    let u = "file:///vault/journals/2024_01_15.md";
    let result = detect_journal_date(u);
    assert_eq!(
        result,
        Some((2024, 1, 15)),
        "expected (2024,1,15) for 2024_01_15.md"
    );
}

#[test]
fn test_journal_date_detected_dash_separator() {
    // Bug this catches: only one separator format supported, ISO 8601 not handled
    let u = "file:///vault/journals/2024-01-15.md";
    let result = detect_journal_date(u);
    assert_eq!(
        result,
        Some((2024, 1, 15)),
        "expected (2024,1,15) for 2024-01-15.md"
    );
}

#[test]
fn test_journal_date_rejected_for_non_journal_filename() {
    // Bug this catches: function matching any file with date-like substring
    let u = "file:///vault/notes/meeting.md";
    assert_eq!(
        detect_journal_date(u),
        None,
        "meeting.md should not be a journal"
    );
}

#[test]
fn test_journal_date_rejected_for_suffix_filename() {
    // Bug this catches: stem length not validated, extra suffix accepted
    let u = "file:///vault/journals/2024_01_15_extra_notes.md";
    assert_eq!(
        detect_journal_date(u),
        None,
        "2024_01_15_extra_notes.md should not match — stem too long"
    );
}

#[test]
fn test_journal_date_rejected_for_mixed_separators() {
    // Bug this catches: accepting inconsistent separator usage (2024-01_15)
    let u = "file:///vault/journals/2024-01_15.md";
    assert_eq!(
        detect_journal_date(u),
        None,
        "mixed separators (2024-01_15.md) should not match"
    );
}

#[test]
fn test_journal_date_rejected_for_invalid_month() {
    // Bug this catches: no range validation, accepting out-of-range months
    let u = "file:///vault/journals/2024_13_01.md";
    assert_eq!(
        detect_journal_date(u),
        None,
        "month=13 is invalid, should return None"
    );
}

#[test]
fn test_realm_indexes_journal_by_date() {
    // Bug this catches: detection runs but result not stored in date_to_docs
    let mut realm = RealmIndex::new();
    let journal_uri = uri("journals/2024_01_15.md");
    realm.add_document(journal_uri.clone(), make_md_index("# Jan 15"));

    let results = realm.lookup_journal_by_month(2024, 1);
    assert_eq!(results.len(), 1, "expected 1 journal doc for Jan 2024");
    assert_eq!(results[0].1, 15u8, "expected day=15");
}

#[test]
fn test_realm_lookup_journal_by_month_multiple() {
    // Bug this catches: BTreeMap range query off by one, returns wrong month
    let mut realm = RealmIndex::new();
    realm.add_document(uri("journals/2024_01_01.md"), make_md_index("day 1"));
    realm.add_document(uri("journals/2024_01_15.md"), make_md_index("day 15"));
    realm.add_document(uri("journals/2024_01_31.md"), make_md_index("day 31"));
    realm.add_document(uri("journals/2024_02_01.md"), make_md_index("feb 1"));
    realm.add_document(uri("journals/2024_02_15.md"), make_md_index("feb 15"));

    let jan = realm.lookup_journal_by_month(2024, 1);
    assert_eq!(jan.len(), 3, "expected 3 Jan docs, got {}", jan.len());

    let feb = realm.lookup_journal_by_month(2024, 2);
    assert_eq!(feb.len(), 2, "expected 2 Feb docs, got {}", feb.len());
}

// ------------------------------------------------------------------
// resolve_relative_path unit tests
// ------------------------------------------------------------------

#[test]
fn test_resolve_relative_path_normal() {
    // From /vault/docs/api → ../guide/overview.md = /vault/docs/guide/overview.md
    let result = resolve_relative_path(
        std::path::Path::new("/vault/docs/api"),
        "../guide/overview.md",
    );
    assert_eq!(
        result,
        std::path::PathBuf::from("/vault/docs/guide/overview.md")
    );
}

#[test]
fn test_resolve_relative_path_dot_underflow_clamps_at_root() {
    // Excessive `..` must not produce a relative path — result must stay absolute.
    // Old code empties the stack and returns a bare filename (relative path).
    // Fixed code clamps at root via PathBuf::pop() semantics.
    let result = resolve_relative_path(std::path::Path::new("/vault"), "../../file.md");
    assert!(
        result.is_absolute(),
        "excessive `..` must not produce a relative path; got: {:?}",
        result
    );
}

#[test]
fn test_resolve_relative_path_single_dot_is_noop() {
    let result = resolve_relative_path(std::path::Path::new("/vault/docs"), "./same.md");
    assert_eq!(result, std::path::PathBuf::from("/vault/docs/same.md"));
}

#[test]
fn test_resolve_relative_path_no_segments() {
    let result = resolve_relative_path(std::path::Path::new("/vault/docs"), "file.md");
    assert_eq!(result, std::path::PathBuf::from("/vault/docs/file.md"));
}

#[test]
fn test_realm_remove_journal_doc_cleans_up_date_index() {
    // Bug this catches: remove_document doesn't clean date_to_docs, causing stale entries
    let mut realm = RealmIndex::new();
    let journal_uri = uri("journals/2024_03_10.md");
    realm.add_document(journal_uri.clone(), make_md_index("day"));
    realm.remove_document(&journal_uri);

    let results = realm.lookup_journal_by_month(2024, 3);
    assert!(
        results.is_empty(),
        "after removal, lookup should return empty — got {} docs",
        results.len()
    );
}

// ── Code span cross-doc index tests ──────────────────────────────────────

fn code_span(text: &str) -> CodeSpanOwned {
    CodeSpanOwned {
        text: text.to_string(),
        range: Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        ),
        start_byte: 0,
        end_byte: text.len(),
    }
}

#[test]
fn test_add_document_populates_code_spans() {
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans("# Intro\n", vec![code_span("HashMap")]);
    realm.add_document(u.clone(), index);

    let results = realm.lookup_code_span("HashMap");
    assert_eq!(results.len(), 1, "expected 1 code span entry");
    assert_eq!(results[0].0, u);
    assert_eq!(results[0].1.text, "HashMap");
}

#[test]
fn test_remove_document_cleans_code_spans() {
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans("# Intro\n", vec![code_span("Vec")]);
    realm.add_document(u.clone(), index);
    assert_eq!(realm.lookup_code_span("Vec").len(), 1);

    realm.remove_document(&u);
    assert!(
        realm.lookup_code_span("Vec").is_empty(),
        "code span should be cleaned up after removal"
    );
}

#[test]
fn test_code_span_dedup_per_document() {
    // Same text 3x in one doc → only 1 entry per doc in cross-doc index.
    let mut realm = RealmIndex::new();
    let u = uri("test.md");
    let index = make_md_index_with_code_spans(
        "# Intro\n",
        vec![
            code_span("Result"),
            code_span("Result"),
            code_span("Result"),
        ],
    );
    realm.add_document(u.clone(), index);

    let results = realm.lookup_code_span("Result");
    assert_eq!(
        results.len(),
        1,
        "same text 3x in one doc should produce 1 entry, got {}",
        results.len()
    );
}

#[test]
fn test_code_span_cross_doc() {
    // Two docs with same code span text → both appear in lookup.
    let mut realm = RealmIndex::new();
    let u1 = uri("a.md");
    let u2 = uri("b.md");
    realm.add_document(
        u1.clone(),
        make_md_index_with_code_spans("# A\n", vec![code_span("Option")]),
    );
    realm.add_document(
        u2.clone(),
        make_md_index_with_code_spans("# B\n", vec![code_span("Option")]),
    );

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

#[test]
fn test_stem_index_basic_lookup() {
    let mut realm = RealmIndex::new();
    let u = uri("notes.md");
    realm.add_document(u.clone(), make_md_index("# Hello"));

    let result = realm.find_uri_by_stem("notes");
    assert_eq!(
        result,
        Some(u),
        "basic stem lookup should find the document"
    );
}

#[test]
fn test_stem_index_case_insensitive() {
    let mut realm = RealmIndex::new();
    let u = DocumentUri::from_file_path(&PathBuf::from("/vault/MyPage.md"));
    realm.add_document(u.clone(), make_md_index("# Page"));

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

#[test]
fn test_stem_index_cross_doc_same_stem() {
    let mut realm = RealmIndex::new();
    let u1 = DocumentUri::from_file_path(&PathBuf::from("/vault/a/readme.md"));
    let u2 = DocumentUri::from_file_path(&PathBuf::from("/vault/b/readme.md"));
    realm.add_document(u1.clone(), make_md_index("# A"));
    realm.add_document(u2.clone(), make_md_index("# B"));

    let result = realm.find_uri_by_stem("readme");
    assert_eq!(
        result,
        Some(u1),
        "same-stem collision should return first-added document"
    );
}

#[test]
fn test_stem_index_remove_then_lookup() {
    let mut realm = RealmIndex::new();
    let u = uri("page.md");
    realm.add_document(u.clone(), make_md_index("# Page"));
    assert!(realm.find_uri_by_stem("page").is_some());

    realm.remove_document(&u);
    assert_eq!(
        realm.find_uri_by_stem("page"),
        None,
        "stem lookup should return None after document removal"
    );
}

#[test]
fn test_stem_index_replace_document() {
    let mut realm = RealmIndex::new();
    let u = uri("page.md");
    realm.add_document(u.clone(), make_md_index("# Old"));
    realm.add_document(u.clone(), make_md_index("# New"));

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

#[test]
fn test_stem_index_remove_one_of_two_same_stem() {
    let mut realm = RealmIndex::new();
    let u1 = DocumentUri::from_file_path(&PathBuf::from("/vault/a/readme.md"));
    let u2 = DocumentUri::from_file_path(&PathBuf::from("/vault/b/readme.md"));
    realm.add_document(u1.clone(), make_md_index("# A"));
    realm.add_document(u2.clone(), make_md_index("# B"));

    realm.remove_document(&u1);

    let result = realm.find_uri_by_stem("readme");
    assert_eq!(
        result,
        Some(u2),
        "after removing first doc, stem should resolve to remaining doc"
    );
}

// ── Layer 3: Incremental cross-doc index updates (marky-c9dm) ──

#[test]
fn test_contribution_built_on_add() {
    let mut realm = RealmIndex::new();
    let u = uri("contrib_add.md");
    realm.add_document(
        u.clone(),
        make_md_index("# Intro\n\n#tag1 #tag2\n\n^block1"),
    );

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

#[test]
fn test_contribution_removed_on_remove() {
    let mut realm = RealmIndex::new();
    let u = uri("contrib_remove.md");
    realm.add_document(u.clone(), make_md_index("# Heading\n\n#mytag"));

    let key = u.as_str().to_string();
    assert!(
        realm.contributions.contains_key(&key),
        "contribution present after add"
    );

    realm.remove_document(&u);
    assert!(
        !realm.contributions.contains_key(&key),
        "contribution removed after remove"
    );
}

#[test]
fn test_update_no_structural_change() {
    let mut realm = RealmIndex::new();
    let u = uri("update_noop.md");
    // Add doc with 3 headings and 2 tags
    realm.add_document(
        u.clone(),
        make_md_index("# Intro\n\n## Details\n\n### Deep\n\n#rust #zig"),
    );

    let heading_count_before = realm
        .slug_to_headings
        .values()
        .map(|v| v.len())
        .sum::<usize>();
    let tag_count_before = realm.tag_to_docs.values().map(|v| v.len()).sum::<usize>();

    // Update with identical structure but different content (simulating range shift)
    realm.update_document(
        u.clone(),
        make_md_index("# Intro\n\n## Details\n\n### Deep\n\n#rust #zig"),
    );

    let heading_count_after = realm
        .slug_to_headings
        .values()
        .map(|v| v.len())
        .sum::<usize>();
    let tag_count_after = realm.tag_to_docs.values().map(|v| v.len()).sum::<usize>();

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

#[test]
fn test_update_heading_added() {
    let mut realm = RealmIndex::new();
    let u = uri("update_heading_add.md");
    realm.add_document(u.clone(), make_md_index("# Intro"));

    // Update: add a second heading
    realm.update_document(u.clone(), make_md_index("# Intro\n\n## Details"));

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

#[test]
fn test_update_heading_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_heading_rm.md");
    realm.add_document(u.clone(), make_md_index("# Intro\n\n## Details"));

    // Update: remove the second heading
    realm.update_document(u.clone(), make_md_index("# Intro"));

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

#[test]
fn test_update_tag_added_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_tags.md");
    realm.add_document(u.clone(), make_md_index("# H\n\n#rust #zig"));

    // Update: remove zig, add wasm
    realm.update_document(u.clone(), make_md_index("# H\n\n#rust #wasm"));

    let rust_spur = realm.interner.get("rust").expect("rust interned");
    let wasm_spur = realm.interner.get("wasm").expect("wasm interned");
    let zig_spur = realm.interner.get("zig").expect("zig interned");

    assert!(
        realm
            .tag_to_docs
            .get(&rust_spur)
            .map(|v| v.contains(&u))
            .unwrap_or(false),
        "rust tag still present"
    );
    assert!(
        realm
            .tag_to_docs
            .get(&wasm_spur)
            .map(|v| v.contains(&u))
            .unwrap_or(false),
        "wasm tag added"
    );
    let has_zig = realm
        .tag_to_docs
        .get(&zig_spur)
        .map(|v| v.contains(&u))
        .unwrap_or(false);
    assert!(!has_zig, "zig tag removed");
}

#[test]
fn test_update_code_span_added() {
    let mut realm = RealmIndex::new();
    let u = uri("update_cs.md");
    realm.add_document(u.clone(), make_md_index("# H"));

    // Update: add a code span
    realm.update_document(
        u.clone(),
        make_md_index_with_code_spans("# H", vec![code_span("HashMap")]),
    );

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

#[test]
fn test_update_block_id_removed() {
    let mut realm = RealmIndex::new();
    let u = uri("update_block.md");
    realm.add_document(u.clone(), make_md_index("# H\n\ntext ^abc"));

    // Update: remove block id
    realm.update_document(u.clone(), make_md_index("# H\n\ntext"));

    let abc_spur = realm.interner.get("abc").expect("abc interned");
    let has_our_doc = realm
        .block_to_location
        .get(&abc_spur)
        .map(|entries| entries.iter().any(|(uri, _)| uri == &u))
        .unwrap_or(false);
    assert!(!has_our_doc, "block id removed for our doc");
}

#[test]
fn test_update_preserves_other_docs_entries() {
    let mut realm = RealmIndex::new();
    let ua = uri("update_a.md");
    let ub = uri("update_b.md");
    // Both docs have heading "intro"
    realm.add_document(ua.clone(), make_md_index("# Intro"));
    realm.add_document(ub.clone(), make_md_index("# Intro"));

    let intro_spur = realm.interner.get("intro").expect("intro interned");
    let before = realm.slug_to_headings.get(&intro_spur).unwrap().len();
    assert_eq!(before, 2, "both docs contribute to intro");

    // Update doc A to remove "intro"
    realm.update_document(ua.clone(), make_md_index("# Changed"));

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
