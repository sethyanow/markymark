//! Completion acceptance tests.

use markymark_core::{DocumentUri, Position};
use markymark_lsp::state::ServerState;

#[test]
fn test_acceptance_completion_updates_after_document_change() {
    // Open a doc, get heading completions, change doc (add heading),
    // get completions again -> new heading appears.
    let mut state = ServerState::new();
    let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(uri_target.clone(), "# Original Heading\n".to_string());
    state.open_document(uri_editor.clone(), "See [[target#".to_string());

    // First completion: should include "Original Heading"
    let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
    assert!(
        !candidates.is_empty(),
        "should return heading completions from target document"
    );
    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"Original Heading"),
        "should include 'Original Heading'; got: {:?}",
        labels
    );

    // Change the target document: add a new heading
    state.change_document(
        &uri_target,
        "# Original Heading\n\n## Added Later\n".to_string(),
    );

    // Second completion: should now include both headings
    let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"Original Heading"),
        "should still include 'Original Heading'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"Added Later"),
        "should include newly added 'Added Later'; got: {:?}",
        labels
    );
}

#[test]
fn test_acceptance_wiki_link_completion_excludes_current_document() {
    // Wiki link completion should NOT suggest the current document.
    // You shouldn't get a suggestion to link to yourself.
    let mut state = ServerState::new();
    let uri_a = DocumentUri::new("file:///test/alpha.md").unwrap();
    let uri_b = DocumentUri::new("file:///test/beta.md").unwrap();

    state.open_document(uri_a.clone(), "# Alpha\n\nLink: [[".to_string());
    state.open_document(uri_b, "# Beta\n".to_string());

    let candidates = state.completion_at(&uri_a, Position::new(2, 8));
    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();

    assert!(
        labels.contains(&"beta"),
        "should include 'beta' from the other document; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"alpha"),
        "should NOT include 'alpha' (current document) in its own completions; got: {:?}",
        labels
    );
}

#[test]
fn test_acceptance_tag_not_triggered_inside_wiki_link() {
    // A `#` inside `[[Page#heading` should be detected as WikiLinkHeading,
    // NOT as a Tag context. The wiki link context takes priority.
    let mut state = ServerState::new();
    let uri = DocumentUri::new("file:///test/doc.md").unwrap();
    state.open_document(uri.clone(), "See [[Page#heading".to_string());

    let ctx = state.detect_completion_context(&uri, Position::new(0, 18));
    assert_eq!(
        ctx,
        Some(markymark_lsp::state::CompletionContext::WikiLinkHeading {
            target: "Page".to_string(),
            partial: "heading".to_string(),
        }),
        "# inside [[ should be WikiLinkHeading, not Tag"
    );
}

#[test]
fn test_acceptance_closed_document_removed_from_completions() {
    // Open 2 docs, verify wiki link completion returns both,
    // close one, verify it no longer appears in completions.
    let mut state = ServerState::new();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();
    let uri_keep = DocumentUri::new("file:///test/keep.md").unwrap();
    let uri_close = DocumentUri::new("file:///test/close-me.md").unwrap();

    state.open_document(uri_editor.clone(), "Link: [[".to_string());
    state.open_document(uri_keep.clone(), "# Keep\n".to_string());
    state.open_document(uri_close.clone(), "# Close Me\n".to_string());

    // Both should appear initially
    let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"keep"),
        "should include 'keep' initially; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"close-me"),
        "should include 'close-me' initially; got: {:?}",
        labels
    );

    // Close one document
    state.close_document(&uri_close);

    // Only 'keep' should remain
    let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"keep"),
        "should still include 'keep' after close; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"close-me"),
        "should NOT include 'close-me' after close; got: {:?}",
        labels
    );
}
