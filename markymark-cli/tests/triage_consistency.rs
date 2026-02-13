//! Validates that the alignment triage document covers all known mismatches.
//!
//! Reads `docs/research/marksman-alignment-triage.md` and checks that every LSP method
//! exercised by the alignment harness has a corresponding triage section with a valid
//! classification (match | intentional divergence | bug).

use std::collections::HashSet;

/// The 8 LSP methods tested by the alignment harness.
const ALIGNMENT_METHODS: &[&str] = &[
    "textDocument/definition",
    "textDocument/references",
    "textDocument/hover",
    "textDocument/completion",
    "textDocument/rename",
    "textDocument/documentSymbol",
    "workspace/symbol",
    "diagnostics",
];

/// Valid triage classifications.
const VALID_CLASSIFICATIONS: &[&str] = &["match", "intentional divergence", "bug"];

fn triage_doc_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("markymark-cli should have parent dir (workspace root)")
        .join("docs/research/marksman-alignment-triage.md")
}

#[test]
fn test_triage_document_exists() {
    let path = triage_doc_path();
    assert!(
        path.exists(),
        "Triage document not found at {}. Run alignment tests and create triage.",
        path.display()
    );
}

#[test]
fn test_triage_covers_all_methods() {
    let path = triage_doc_path();
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read triage doc: {e}"));

    let content_lower = content.to_lowercase();

    let mut missing = Vec::new();
    for method in ALIGNMENT_METHODS {
        // Check for the method name appearing in the document (case-insensitive for diagnostics)
        let method_lower = method.to_lowercase();
        if !content_lower.contains(&method_lower) {
            missing.push(*method);
        }
    }

    assert!(
        missing.is_empty(),
        "Triage document is missing coverage for methods: {:?}\nEvery alignment method must be triaged.",
        missing,
    );
}

#[test]
fn test_triage_has_valid_classifications() {
    let path = triage_doc_path();
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read triage doc: {e}"));

    let content_lower = content.to_lowercase();

    // Find all "Classification:" lines (may be wrapped in markdown bold **)
    let classifications: Vec<&str> = content_lower
        .lines()
        .filter(|line| {
            let stripped = line.replace('*', "");
            stripped.contains("classification:")
        })
        .collect();

    assert!(
        !classifications.is_empty(),
        "Triage document has no classification lines. Each mismatch must have a classification.",
    );

    let valid_set: HashSet<&str> = VALID_CLASSIFICATIONS.iter().copied().collect();

    for line in &classifications {
        let stripped = line.replace('*', "");
        let has_valid = valid_set.iter().any(|cls| stripped.contains(cls));
        assert!(
            has_valid,
            "Invalid classification in line: '{}'. Valid: {:?}",
            line.trim(),
            VALID_CLASSIFICATIONS,
        );
    }
}

#[test]
fn test_triage_no_placeholder_todos() {
    let path = triage_doc_path();
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read triage doc: {e}"));

    let content_lower = content.to_lowercase();

    assert!(
        !content_lower.contains("todo"),
        "Triage document contains TODO placeholders. All sections must be complete.",
    );
    assert!(
        !content_lower.contains("tbd"),
        "Triage document contains TBD placeholders. All sections must be complete.",
    );
    assert!(
        !content_lower.contains("fixme"),
        "Triage document contains FIXME placeholders. All sections must be complete.",
    );
}

#[test]
fn test_triage_summary_table_present() {
    let path = triage_doc_path();
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read triage doc: {e}"));

    assert!(
        content.contains("## Summary"),
        "Triage document must have a Summary section.",
    );
    assert!(
        content.contains("| Classification"),
        "Triage document must have a classification summary table.",
    );
}
