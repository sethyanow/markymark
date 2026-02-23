//! Completion result tests.

use markymark_core::{DocumentUri, Position};
use markymark_lsp::state::{CompletionCandidateKind, ServerState};

#[test]
fn test_wiki_link_completion_returns_page_names() {
    // Open 3 documents, complete inside `[[` -> returns all 3 page names.
    let mut state = ServerState::new();
    let uri_notes = DocumentUri::new("file:///test/notes.md").unwrap();
    let uri_readme = DocumentUri::new("file:///test/readme.md").unwrap();
    let uri_todo = DocumentUri::new("file:///test/todo.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(uri_notes, "# Notes\n".to_string());
    state.open_document(uri_readme, "# Readme\n".to_string());
    state.open_document(uri_todo, "# Todo\n".to_string());
    // The editing document triggers completion
    state.open_document(uri_editor.clone(), "Link to [[".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 10));
    assert!(
        !candidates.is_empty(),
        "wiki link completion should return page names"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"notes"),
        "should include 'notes' in completions; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"readme"),
        "should include 'readme' in completions; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"todo"),
        "should include 'todo' in completions; got: {:?}",
        labels
    );

    // All should be Page kind
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == CompletionCandidateKind::Page),
        "all wiki link completions should be Page kind"
    );
}

#[test]
fn test_wiki_link_completion_filters_by_partial() {
    // Open 3 documents, complete `[[no` -> returns only "notes".
    let mut state = ServerState::new();
    let uri_notes = DocumentUri::new("file:///test/notes.md").unwrap();
    let uri_readme = DocumentUri::new("file:///test/readme.md").unwrap();
    let uri_todo = DocumentUri::new("file:///test/todo.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(uri_notes, "# Notes\n".to_string());
    state.open_document(uri_readme, "# Readme\n".to_string());
    state.open_document(uri_todo, "# Todo\n".to_string());
    state.open_document(uri_editor.clone(), "Link to [[no".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 12));
    assert!(
        !candidates.is_empty(),
        "wiki link completion with partial 'no' should return matches"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"notes"),
        "should include 'notes'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"readme"),
        "should NOT include 'readme'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"todo"),
        "should NOT include 'todo'; got: {:?}",
        labels
    );
}

#[test]
fn test_heading_completion_returns_target_headings() {
    // Open a target document with headings, complete `[[target#` -> returns headings.
    let mut state = ServerState::new();
    let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_target,
        "# Introduction\n\n## Getting Started\n\n## Advanced Topics\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "See [[target#".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
    assert!(
        !candidates.is_empty(),
        "heading completion should return headings from target document"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"Introduction"),
        "should include 'Introduction'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"Getting Started"),
        "should include 'Getting Started'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"Advanced Topics"),
        "should include 'Advanced Topics'; got: {:?}",
        labels
    );

    // All should be Heading kind
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == CompletionCandidateKind::Heading),
        "all heading completions should be Heading kind"
    );
}

#[test]
fn test_heading_completion_filters_by_partial() {
    // Complete `[[target#int` -> returns only headings containing "int".
    let mut state = ServerState::new();
    let uri_target = DocumentUri::new("file:///test/target.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_target,
        "# Introduction\n\n## Getting Started\n\n## Advanced Topics\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "See [[target#int".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 16));
    assert!(
        !candidates.is_empty(),
        "heading completion with partial 'int' should return matches"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.to_lowercase().contains("int")),
        "should include heading matching 'int'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"Getting Started"),
        "should NOT include 'Getting Started'; got: {:?}",
        labels
    );
}

#[test]
fn test_tag_completion_returns_tags() {
    // Open a document with tags, complete `#` -> returns available tags.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "Some text with #rust and #programming tags.\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "Tags: #".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 7));
    assert!(
        !candidates.is_empty(),
        "tag completion should return available tags"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"rust"),
        "should include 'rust'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"programming"),
        "should include 'programming'; got: {:?}",
        labels
    );

    // All should be Tag kind
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == CompletionCandidateKind::Tag),
        "all tag completions should be Tag kind"
    );
}

#[test]
fn test_tag_completion_filters_by_partial() {
    // Complete `#pro` -> returns only matching tags.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "Some text with #rust and #programming tags.\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "Tags: #pro".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 10));
    assert!(
        !candidates.is_empty(),
        "tag completion with partial 'pro' should return matches"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"programming"),
        "should include 'programming'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"rust"),
        "should NOT include 'rust'; got: {:?}",
        labels
    );
}

#[test]
fn test_block_ref_completion_returns_block_ids() {
    // Open a document with block IDs, complete `((` -> returns block IDs.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "Some paragraph ^abc123\n\nAnother paragraph ^def456\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "Ref ((".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 6));
    assert!(
        !candidates.is_empty(),
        "block ref completion should return block IDs"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"abc123"),
        "should include 'abc123'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"def456"),
        "should include 'def456'; got: {:?}",
        labels
    );

    // All should be BlockRef kind
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == CompletionCandidateKind::BlockRef),
        "all block ref completions should be BlockRef kind"
    );
}

#[test]
fn test_block_ref_completion_filters_by_partial() {
    // Complete `((ab` -> returns only matching block IDs.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "Some paragraph ^abc123\n\nAnother paragraph ^def456\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "Ref ((ab".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 8));
    assert!(
        !candidates.is_empty(),
        "block ref completion with partial 'ab' should return matches"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"abc123"),
        "should include 'abc123'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"def456"),
        "should NOT include 'def456'; got: {:?}",
        labels
    );
}

#[test]
fn test_xml_tag_completion_returns_tag_names() {
    // Open documents with XML tags, complete `<` -> returns known tag names.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n\n<routing>\n\npath\n\n</routing>\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "New content <".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 13));
    assert!(
        !candidates.is_empty(),
        "XML tag completion should return known tag names"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"agent"),
        "should include 'agent'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"goal"),
        "should include 'goal'; got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"routing"),
        "should include 'routing'; got: {:?}",
        labels
    );

    // All should be XmlTag kind
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == CompletionCandidateKind::XmlTag),
        "all XML tag completions should be XmlTag kind"
    );
}

#[test]
fn test_xml_tag_completion_filters_by_partial() {
    // Complete `<ag` -> returns only matching tag names.
    let mut state = ServerState::new();
    let uri_source = DocumentUri::new("file:///test/source.md").unwrap();
    let uri_editor = DocumentUri::new("file:///test/editor.md").unwrap();

    state.open_document(
        uri_source,
        "<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n".to_string(),
    );
    state.open_document(uri_editor.clone(), "New <ag".to_string());

    let candidates = state.completion_at(&uri_editor, Position::new(0, 7));
    assert!(
        !candidates.is_empty(),
        "XML tag completion with partial 'ag' should return matches"
    );

    let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"agent"),
        "should include 'agent'; got: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"goal"),
        "should NOT include 'goal'; got: {:?}",
        labels
    );
}
