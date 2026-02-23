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
