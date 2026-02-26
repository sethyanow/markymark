//! Diagnostics tests for broken links and duplicate headings.

use markymark_core::DocumentUri;
use markymark_lsp::state::{DiagnosticSeverity, MarkyDiagnostic, ServerState};

// =======================================================================
// Unit tests: state-level diagnostics
// =======================================================================

#[tokio::test]
async fn test_no_diagnostics_for_valid_document() {
    // A document where all links resolve and no duplicate slugs -> empty diagnostics.
    let mut state = ServerState::new();
    let uri_main = DocumentUri::new("file:///test/main.md").unwrap();
    let uri_other = DocumentUri::new("file:///test/other-page.md").unwrap();

    state
        .open_document(
            uri_other.clone(),
            "# Other Page\n\n## Details\n".to_string(),
        )
        .await;
    state
        .open_document(
            uri_main.clone(),
            concat!(
                "# Main\n",
                "\n",
                "## Introduction\n",
                "\n",
                "See [[other-page]] for info.\n",
                "\n",
                "Check [[other-page#details]] and [[#introduction]].\n",
                "\n",
                "A markdown anchor: [intro](#introduction)\n",
            )
            .to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri_main);
    assert!(
        diagnostics.is_empty(),
        "valid document should produce no diagnostics, got: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_broken_wiki_link_to_nonexistent_page() {
    // [[nonexistent]] where no document with stem "nonexistent" exists -> Error.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Doc\n\nSee [[nonexistent]] here.\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    assert_eq!(
        diagnostics.len(),
        1,
        "should produce exactly one diagnostic for broken wiki link"
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0].message.contains("nonexistent"),
        "diagnostic message should mention the broken target; got: {}",
        diagnostics[0].message
    );
}

#[tokio::test]
async fn test_broken_wiki_link_to_nonexistent_heading() {
    // [[other-page#nonexistent]] where other-page exists but heading doesn't -> Error.
    let mut state = ServerState::new();
    let uri_main = DocumentUri::new("file:///test/main.md").unwrap();
    let uri_other = DocumentUri::new("file:///test/other-page.md").unwrap();

    state
        .open_document(
            uri_other.clone(),
            "# Other Page\n\n## Details\n".to_string(),
        )
        .await;
    state
        .open_document(
            uri_main.clone(),
            "# Main\n\nSee [[other-page#nonexistent]].\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri_main);
    assert_eq!(
        diagnostics.len(),
        1,
        "should produce exactly one diagnostic for broken heading ref"
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0].message.contains("other-page#nonexistent"),
        "message should reference the full target; got: {}",
        diagnostics[0].message
    );
}

#[tokio::test]
async fn test_broken_same_page_wiki_link() {
    // [[#nonexistent]] where the heading doesn't exist in the same doc -> Error.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Document\n\nSee [[#nonexistent]] above.\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    assert_eq!(
        diagnostics.len(),
        1,
        "should produce exactly one diagnostic for broken same-page wiki link"
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0].message.contains("nonexistent"),
        "message should mention the missing heading; got: {}",
        diagnostics[0].message
    );
}

#[tokio::test]
async fn test_broken_markdown_link_anchor() {
    // [text](#nonexistent) where heading slug doesn't exist -> Error.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Document\n\nSee [text](#nonexistent) here.\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    assert_eq!(
        diagnostics.len(),
        1,
        "should produce exactly one diagnostic for broken markdown anchor"
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0].message.contains("nonexistent"),
        "message should mention the missing anchor; got: {}",
        diagnostics[0].message
    );
}

#[tokio::test]
async fn test_duplicate_heading_slugs() {
    // Two headings that produce the same slug -> Warning for both.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            concat!(
                "# Document\n",
                "\n",
                "## Details\n",
                "\n",
                "Some content.\n",
                "\n",
                "## Details\n",
                "\n",
                "More content.\n",
            )
            .to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    let warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();

    // The implementation emits a warning for EACH occurrence of a duplicate slug.
    // With 2 headings sharing the slug "details", we expect 2 warnings.
    assert_eq!(
        warnings.len(),
        2,
        "should produce a warning for each duplicate-slug heading; got {} warnings",
        warnings.len()
    );

    for w in &warnings {
        assert!(
            w.message.contains("details"),
            "warning message should mention the duplicate slug; got: {}",
            w.message
        );
        assert!(
            w.message.contains("2 occurrences"),
            "warning should state the occurrence count; got: {}",
            w.message
        );
    }
}

#[tokio::test]
async fn test_valid_wiki_link_no_diagnostic() {
    // [[other-page]] that resolves -> no diagnostics for that link.
    let mut state = ServerState::new();
    let uri_main = DocumentUri::new("file:///test/main.md").unwrap();
    let uri_other = DocumentUri::new("file:///test/other-page.md").unwrap();

    state
        .open_document(uri_other.clone(), "# Other Page\n".to_string())
        .await;
    state
        .open_document(
            uri_main.clone(),
            "# Main\n\nSee [[other-page]].\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri_main);
    assert!(
        diagnostics.is_empty(),
        "valid wiki link should produce no diagnostics, got: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_valid_markdown_link_anchor_no_diagnostic() {
    // [text](#existing-heading) where the heading exists -> no diagnostics.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Document\n\n## My Section\n\nSee [link](#my-section) above.\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    assert!(
        diagnostics.is_empty(),
        "valid markdown anchor should produce no diagnostics, got: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_multiple_diagnostics_in_same_document() {
    // A document with both broken links AND duplicate headings -> multiple diagnostics.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            concat!(
                "# Document\n",
                "\n",
                "## Section\n",
                "\n",
                "See [[nonexistent-page]].\n",
                "\n",
                "Check [link](#no-such-heading).\n",
                "\n",
                "## Section\n",
                "\n",
                "Duplicate heading above.\n",
            )
            .to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);

    let errors: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    let warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();

    // Broken wiki link + broken markdown anchor = 2 errors
    assert!(
        errors.len() >= 2,
        "should have at least 2 error diagnostics (broken wiki link + broken anchor); got {}",
        errors.len()
    );

    // 2 headings with slug "section" = 2 warnings
    assert!(
        warnings.len() >= 2,
        "should have at least 2 warning diagnostics (duplicate heading slugs); got {}",
        warnings.len()
    );

    // Verify total count
    assert!(
        diagnostics.len() >= 4,
        "should have at least 4 diagnostics total; got {}",
        diagnostics.len()
    );
}

#[tokio::test]
async fn test_diagnostics_for_unknown_document() {
    // URI not in realm -> empty diagnostics (not an error).
    let state = ServerState::new();
    let uri = DocumentUri::new("file:///test/unknown.md").unwrap();

    let diagnostics = state.compute_diagnostics(&uri);
    assert!(
        diagnostics.is_empty(),
        "unknown document should produce empty diagnostics, not panic"
    );
}

// =======================================================================
// Acceptance tests: diagnostics lifecycle
// =======================================================================

#[tokio::test]
async fn test_diagnostics_update_after_document_change() {
    // Open doc with broken link -> has diagnostic.
    // Change doc to fix link -> diagnostic gone.
    let mut state = ServerState::new();
    let uri_main = DocumentUri::new("file:///test/main.md").unwrap();
    let uri_target = DocumentUri::new("file:///test/target.md").unwrap();

    // Open a target page so we can link to it later.
    state
        .open_document(uri_target.clone(), "# Target\n".to_string())
        .await;

    // Open main with a broken link.
    state
        .open_document(
            uri_main.clone(),
            "# Main\n\nSee [[nonexistent]].\n".to_string(),
        )
        .await;

    let diag_before = state.compute_diagnostics(&uri_main);
    assert_eq!(
        diag_before.len(),
        1,
        "should have 1 diagnostic for broken link before fix"
    );
    assert_eq!(diag_before[0].severity, DiagnosticSeverity::Error);

    // Fix the link by changing the document content.
    state
        .change_document(&uri_main, "# Main\n\nSee [[target]].\n".to_string())
        .await;

    let diag_after = state.compute_diagnostics(&uri_main);
    assert!(
        diag_after.is_empty(),
        "after fixing the link, diagnostics should be empty; got: {:?}",
        diag_after.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_diagnostics_cleared_after_close() {
    // Open doc with broken link -> has diagnostic.
    // Close doc -> diagnostics should be empty (document no longer in realm).
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();

    state
        .open_document(uri.clone(), "# Doc\n\nSee [[nowhere]].\n".to_string())
        .await;

    let diag_before = state.compute_diagnostics(&uri);
    assert_eq!(
        diag_before.len(),
        1,
        "should have 1 diagnostic before close"
    );

    // Close the document.
    state.close_document(&uri);

    let diag_after = state.compute_diagnostics(&uri);
    assert!(
        diag_after.is_empty(),
        "after close, diagnostics should be empty (doc removed from realm)"
    );
}

// =======================================================================
// XML tag diagnostics
// =======================================================================

#[tokio::test]
async fn test_unclosed_xml_tag_produces_warning() {
    // <agent> without </agent> -> Warning
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(uri.clone(), "# Doc\n\n<agent>\nSome content\n".to_string())
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    let warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();

    assert_eq!(
        warnings.len(),
        1,
        "should produce 1 warning for unclosed <agent> tag; got: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        warnings[0].message.contains("agent"),
        "warning should mention the tag name; got: {}",
        warnings[0].message
    );
}

#[tokio::test]
async fn test_closed_xml_tag_no_diagnostic() {
    // <agent>...</agent> -> no diagnostic
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Doc\n\n<agent>\nContent\n</agent>\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    let xml_warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.message.contains("Unclosed"))
        .collect();
    assert!(
        xml_warnings.is_empty(),
        "properly closed tag should produce no XML diagnostics; got: {:?}",
        xml_warnings.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_self_closing_xml_tag_no_diagnostic() {
    // <br/> -> no diagnostic (self-closing is valid)
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            "# Doc\n\n<config type=\"test\"/>\n".to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    let xml_warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.message.contains("Unclosed"))
        .collect();
    assert!(
        xml_warnings.is_empty(),
        "self-closing tag should produce no XML diagnostics; got: {:?}",
        xml_warnings.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_xml_like_syntax_inside_fenced_code_produces_no_xml_warning() {
    // Rust generics inside fenced code should not trigger XML diagnostics.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state
        .open_document(
            uri.clone(),
            concat!(
                "# Doc\n\n",
                "```rust\n",
                "fn wrap<T>(value: T) -> Arc<Mutex<T>> { value }\n",
                "```\n\n",
                "~~~rust\n",
                "fn use_dyn(v: Box<dyn std::fmt::Display>) {}\n",
                "~~~\n",
            )
            .to_string(),
        )
        .await;

    let diagnostics = state.compute_diagnostics(&uri);
    let xml_warnings: Vec<&MarkyDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.message.contains("Unclosed XML tag"))
        .collect();

    assert!(
        xml_warnings.is_empty(),
        "fenced code generics should not produce XML warnings; got: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}
