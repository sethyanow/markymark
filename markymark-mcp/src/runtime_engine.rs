use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_index::{slugify, DocumentIndex, MarkdownLinkEntry, RealmIndex, WikiLinkEntry};
use markymark_parser::Parser;

/// Production core engine backed by an indexed set of markdown workspace roots.
#[derive(Default)]
pub struct RuntimeEngine {
    realm: RealmIndex,
}

impl RuntimeEngine {
    /// Build a runtime engine from workspace roots.
    ///
    /// All markdown files (`*.md`, `*.markdown`) under the provided roots are indexed.
    /// Invalid roots fail startup. Individual document read/parse failures are skipped.
    pub fn from_workspace_roots(workspace_roots: Vec<PathBuf>) -> anyhow::Result<Self> {
        if workspace_roots.is_empty() {
            bail!("at least one workspace root is required");
        }

        let mut parser = Parser::new().map_err(|err| anyhow!(err.to_string()))?;
        let mut realm = RealmIndex::new();

        for root in workspace_roots {
            validate_workspace_root(&root)?;
            let markdown_files = collect_markdown_files(&root);

            for path in markdown_files {
                let source = match fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(_) => continue,
                };

                let ast = match parser.parse(&source) {
                    Ok(ast) => ast,
                    Err(_) => continue,
                };

                realm.add_document(
                    DocumentUri::from_file_path(&path),
                    DocumentIndex::from_ast(&ast),
                );
            }
        }

        Ok(Self { realm })
    }
}

impl RuntimeEngine {
    /// Read the source text for a document from disk.
    fn read_document_text(uri: &DocumentUri) -> Option<String> {
        let path = uri.to_file_path()?;
        fs::read_to_string(path).ok()
    }

    /// Rename a heading and all references to it.
    fn rename_heading(
        &self,
        uri: &DocumentUri,
        heading: markymark_index::HeadingEntry,
        new_name: &str,
    ) -> CoreOperationResult {
        let old_slug = heading.slug.clone();
        let new_slug = slugify(new_name);
        let mut doc_edits: std::collections::HashMap<DocumentUri, Vec<(Range, String)>> =
            std::collections::HashMap::new();

        // 1. Edit the heading text itself.
        //    Skip the "## " prefix to find the text-only range.
        if let Some(text) = Self::read_document_text(uri) {
            if let Some(heading_line) = text.lines().nth(heading.range.start.line as usize) {
                let prefix_len =
                    heading_line.len() - heading_line.trim_start_matches('#').trim_start().len();
                let text_start = Position::new(heading.range.start.line, prefix_len as u32);
                let text_end = Position::new(
                    heading.range.start.line,
                    prefix_len as u32 + heading.text.len() as u32,
                );
                doc_edits
                    .entry(uri.clone())
                    .or_default()
                    .push((Range::new(text_start, text_end), new_name.to_string()));
            }
        }

        // 2. Update wiki links and markdown link anchors across all documents.
        for (doc_uri, doc_index) in self.realm.iter_documents() {
            let doc_text = Self::read_document_text(doc_uri);

            for wl in doc_index.wiki_links() {
                if wl.heading.as_deref() == Some(&old_slug) {
                    if let Some(anchor_range) =
                        find_wiki_link_heading_range(doc_text.as_deref(), wl, &old_slug)
                    {
                        doc_edits
                            .entry(doc_uri.clone())
                            .or_default()
                            .push((anchor_range, new_name.to_string()));
                    }
                }
            }

            for ml in doc_index.markdown_links() {
                if ml.anchor.as_deref() == Some(&old_slug) {
                    if let Some(anchor_range) =
                        find_markdown_link_anchor_range(doc_text.as_deref(), ml, &old_slug)
                    {
                        doc_edits
                            .entry(doc_uri.clone())
                            .or_default()
                            .push((anchor_range, new_slug.clone()));
                    }
                }
            }
        }

        // Convert to CoreOperationResult::WorkspaceEdit format, sorted deterministically.
        let mut result: Vec<(DocumentUri, Vec<(Range, String)>)> = doc_edits
            .into_iter()
            .map(|(uri, mut edits)| {
                edits.sort_by(|(a, _), (b, _)| compare_ranges(*a, *b));
                (uri, edits)
            })
            .collect();
        result.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));

        CoreOperationResult::WorkspaceEdit(result)
    }

    /// Rename an XML tag across all documents.
    fn rename_xml_tag(&self, old_name: &str, new_name: &str) -> CoreOperationResult {
        let mut doc_edits: std::collections::HashMap<DocumentUri, Vec<(Range, String)>> =
            std::collections::HashMap::new();

        for (doc_uri, doc_index) in self.realm.iter_documents() {
            for xml in doc_index.xml_tags() {
                if xml.tag_name == old_name {
                    // Opening tag name: starts after '<'
                    let name_start =
                        Position::new(xml.range.start.line, xml.range.start.character + 1);
                    let name_end = Position::new(
                        xml.range.start.line,
                        xml.range.start.character + 1 + xml.tag_name.len() as u32,
                    );
                    doc_edits
                        .entry(doc_uri.clone())
                        .or_default()
                        .push((Range::new(name_start, name_end), new_name.to_string()));

                    // Closing tag name (skip for self-closing and unclosed)
                    if !xml.is_self_closing && !xml.is_unclosed {
                        let close_name_start = Position::new(
                            xml.range.end.line,
                            xml.range.end.character - 1 - xml.tag_name.len() as u32,
                        );
                        let close_name_end =
                            Position::new(xml.range.end.line, xml.range.end.character - 1);
                        doc_edits.entry(doc_uri.clone()).or_default().push((
                            Range::new(close_name_start, close_name_end),
                            new_name.to_string(),
                        ));
                    }
                }
            }
        }

        if doc_edits.is_empty() {
            return CoreOperationResult::Error(CoreError::Message(
                "no renameable symbol at position".to_string(),
            ));
        }

        let mut result: Vec<(DocumentUri, Vec<(Range, String)>)> = doc_edits
            .into_iter()
            .map(|(uri, mut edits)| {
                edits.sort_by(|(a, _), (b, _)| compare_ranges(*a, *b));
                (uri, edits)
            })
            .collect();
        result.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));

        CoreOperationResult::WorkspaceEdit(result)
    }
}

impl CoreEngine for RuntimeEngine {
    fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            CoreOperation::GetOutline { uri } => match self.realm.get_document(&uri) {
                Some(index) => CoreOperationResult::Outline(
                    index
                        .headings()
                        .iter()
                        .map(|heading| heading.text.clone())
                        .collect(),
                ),
                None => CoreOperationResult::Error(CoreError::Message(format!(
                    "document is not indexed: {}",
                    uri.as_str()
                ))),
            },
            CoreOperation::SearchSymbols { query } => {
                let query = query.trim();
                if query.is_empty() {
                    return CoreOperationResult::Error(CoreError::Message(
                        "search query cannot be empty".to_string(),
                    ));
                }

                let mut matches = Vec::new();
                let query_lower = query.to_lowercase();

                for (uri, index) in self.realm.iter_documents() {
                    for heading in index.headings() {
                        if heading.text.to_lowercase().contains(&query_lower) {
                            matches.push((heading.text.clone(), uri.clone(), heading.range));
                        }
                    }
                }

                matches.sort_by(|(name_a, uri_a, range_a), (name_b, uri_b, range_b)| {
                    name_a
                        .cmp(name_b)
                        .then_with(|| uri_a.as_str().cmp(uri_b.as_str()))
                        .then_with(|| compare_ranges(*range_a, *range_b))
                });

                CoreOperationResult::Symbols(matches)
            }
            CoreOperation::FindReferences { uri, position } => {
                let index = match self.realm.get_document(&uri) {
                    Some(idx) => idx,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "document is not indexed: {}",
                            uri.as_str()
                        )));
                    }
                };

                // Use the start of the position range as the cursor point.
                let cursor = position.start;

                // Identify the symbol at the cursor: headings and XML tags support references.
                if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                    let slug = &heading.slug;
                    let mut locations = Vec::new();

                    for (doc_uri, doc_index) in self.realm.iter_documents() {
                        for wl in doc_index.wiki_links() {
                            if wl.heading.as_deref() == Some(slug) {
                                locations.push((doc_uri.clone(), wl.range));
                            }
                        }
                        for ml in doc_index.markdown_links() {
                            if ml.anchor.as_deref() == Some(slug) {
                                locations.push((doc_uri.clone(), ml.range));
                            }
                        }
                    }

                    locations.sort_by(|(uri_a, range_a), (uri_b, range_b)| {
                        uri_a
                            .as_str()
                            .cmp(uri_b.as_str())
                            .then_with(|| compare_ranges(*range_a, *range_b))
                    });

                    return CoreOperationResult::Locations(locations);
                }

                if let Some(xml_tag) = index.xml_tags().iter().find(|x| x.range.contains(cursor)) {
                    let tag_name = &xml_tag.tag_name;
                    let mut locations = Vec::new();

                    for (doc_uri, doc_index) in self.realm.iter_documents() {
                        for xt in doc_index.xml_tags() {
                            if xt.tag_name == *tag_name {
                                locations.push((doc_uri.clone(), xt.range));
                            }
                        }
                    }

                    locations.sort_by(|(uri_a, range_a), (uri_b, range_b)| {
                        uri_a
                            .as_str()
                            .cmp(uri_b.as_str())
                            .then_with(|| compare_ranges(*range_a, *range_b))
                    });

                    return CoreOperationResult::Locations(locations);
                }

                CoreOperationResult::Error(CoreError::Message(
                    "no referenceable symbol at position".to_string(),
                ))
            }
            CoreOperation::Rename {
                uri,
                position,
                new_name,
            } => {
                let index = match self.realm.get_document(&uri) {
                    Some(idx) => idx,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "document is not indexed: {}",
                            uri.as_str()
                        )));
                    }
                };

                let cursor = position.start;

                // Heading rename
                if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                    return self.rename_heading(&uri, heading.clone(), &new_name);
                }

                // XML tag rename
                if let Some(xml_tag) = index.xml_tags().iter().find(|x| x.range.contains(cursor)) {
                    return self.rename_xml_tag(&xml_tag.tag_name, &new_name);
                }

                CoreOperationResult::Error(CoreError::Message(
                    "no renameable symbol at position".to_string(),
                ))
            }
        }
    }
}

fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
}

fn validate_workspace_root(root: &Path) -> anyhow::Result<()> {
    if !root.exists() {
        bail!("workspace root does not exist: {}", root.display());
    }
    if !root.is_dir() {
        bail!("workspace root is not a directory: {}", root.display());
    }
    Ok(())
}

fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = stack.pop() {
        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(_) => continue,
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();

            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if is_markdown_path(&path) {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

/// Find the range of the heading portion within a wiki link.
///
/// Given a wiki link like `[[page#heading]]` or `[[#heading]]`, returns the
/// range covering just the heading text (after `#`, before `]]`).
fn find_wiki_link_heading_range(
    doc_text: Option<&str>,
    wl: &WikiLinkEntry,
    old_heading: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(wl.range.start.line as usize)?;
    let link_start = wl.range.start.character as usize;
    let link_text = &line[link_start..];

    let hash_offset = link_text.find('#')?;
    let heading_start = link_start + hash_offset + 1; // skip the '#'
    let heading_end = heading_start + old_heading.len();

    if line.get(heading_start..heading_end) == Some(old_heading) {
        Some(Range::new(
            Position::new(wl.range.start.line, heading_start as u32),
            Position::new(wl.range.start.line, heading_end as u32),
        ))
    } else {
        None
    }
}

/// Find the range of the anchor portion within a markdown link.
///
/// Given a markdown link like `[text](#slug)`, returns the range covering
/// just the slug text (after `#`, before `)`).
fn find_markdown_link_anchor_range(
    doc_text: Option<&str>,
    ml: &MarkdownLinkEntry,
    old_slug: &str,
) -> Option<Range> {
    let text = doc_text?;
    let line = text.lines().nth(ml.range.start.line as usize)?;
    let link_start = ml.range.start.character as usize;
    let link_text = &line[link_start..];

    let paren_hash = link_text.find("(#")?;
    let slug_start = link_start + paren_hash + 2; // skip "(#"
    let slug_end = slug_start + old_slug.len();

    if line.get(slug_start..slug_end) == Some(old_slug) {
        Some(Range::new(
            Position::new(ml.range.start.line, slug_start as u32),
            Position::new(ml.range.start.line, slug_end as u32),
        ))
    } else {
        None
    }
}

fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md") | Some("markdown")
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use markymark_core::engine::CoreOperation;
    use markymark_core::Position;

    use super::*;

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "markymark-mcp-runtime-{name}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("temporary workspace directory should be created");
            Self { root }
        }

        fn root(&self) -> PathBuf {
            self.root.clone()
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn rejects_empty_workspace_roots() {
        let err = match RuntimeEngine::from_workspace_roots(Vec::new()) {
            Ok(_) => panic!("empty workspace roots should fail"),
            Err(err) => err,
        };
        assert!(err
            .to_string()
            .contains("at least one workspace root is required"));
    }

    #[test]
    fn rejects_missing_workspace_root() {
        let missing = std::env::temp_dir().join("markymark-missing-workspace");
        if missing.exists() {
            fs::remove_dir_all(&missing).expect("stale missing-workspace path should be removable");
        }

        let err = match RuntimeEngine::from_workspace_roots(vec![missing.clone()]) {
            Ok(_) => panic!("missing workspace root should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains(&format!(
            "workspace root does not exist: {}",
            missing.display()
        )));
    }

    #[test]
    fn rejects_workspace_root_file_path() {
        let ws = TempWorkspace::new("root-file");
        let file_path = ws.root().join("not-a-directory.md");
        fs::write(&file_path, "# Heading").expect("test file should be created");

        let err = match RuntimeEngine::from_workspace_roots(vec![file_path.clone()]) {
            Ok(_) => panic!("workspace root file should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains(&format!(
            "workspace root is not a directory: {}",
            file_path.display()
        )));
    }

    #[test]
    fn indexes_markdown_and_returns_deterministic_symbols() {
        let ws = TempWorkspace::new("indexed");
        let first = ws.root().join("a.md");
        let second = ws.root().join("b.md");
        fs::write(&first, "# Zebra\n## Alpha\n").expect("first markdown should be created");
        fs::write(&second, "# Beta\n").expect("second markdown should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let outline = engine.execute(CoreOperation::GetOutline {
            uri: DocumentUri::from_file_path(&first),
        });
        match outline {
            CoreOperationResult::Outline(headings) => {
                assert_eq!(headings, vec!["Zebra".to_string(), "Alpha".to_string()]);
            }
            other => panic!("expected outline result, got: {other:?}"),
        }

        let symbols = engine.execute(CoreOperation::SearchSymbols {
            query: "a".to_string(),
        });
        match symbols {
            CoreOperationResult::Symbols(matches) => {
                let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
                assert_eq!(
                    names,
                    vec!["Alpha".to_string(), "Beta".to_string(), "Zebra".to_string()]
                );
            }
            other => panic!("expected symbol matches, got: {other:?}"),
        }
    }

    #[test]
    fn find_references_returns_wiki_link_refs_to_heading() {
        let ws = TempWorkspace::new("find-refs-heading");
        // a.md has a heading "## Setup" and a wiki link to it
        let a = ws.root().join("a.md");
        fs::write(&a, "# Title\n\n## Setup\n\nSee [[#setup]] for info.\n")
            .expect("a.md should be created");
        // b.md has a wiki link referencing the same heading slug
        let b = ws.root().join("b.md");
        fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Position cursor on the "## Setup" heading (line 2, char 3 = within "Setup")
        let result = engine.execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 3), Position::new(2, 3)),
        });

        match result {
            CoreOperationResult::Locations(locations) => {
                // Should find at least the wiki link in a.md and the cross-file wiki link in b.md
                assert!(
                    locations.len() >= 2,
                    "expected at least 2 references, got {}",
                    locations.len()
                );
                // Verify deterministic ordering: sorted by URI then range
                for window in locations.windows(2) {
                    let (uri_a, range_a) = &window[0];
                    let (uri_b, range_b) = &window[1];
                    let ord = uri_a
                        .as_str()
                        .cmp(uri_b.as_str())
                        .then_with(|| compare_ranges(*range_a, *range_b));
                    assert!(
                        ord != Ordering::Greater,
                        "locations should be sorted, but {uri_a:?} > {uri_b:?}"
                    );
                }
            }
            other => panic!("expected Locations result, got: {other:?}"),
        }
    }

    #[test]
    fn find_references_returns_markdown_link_refs_to_heading() {
        let ws = TempWorkspace::new("find-refs-mdlink");
        let a = ws.root().join("a.md");
        // "## My Section" slug = "my-section"
        // Markdown link [link](#my-section) references it
        fs::write(
            &a,
            "# Title\n\n## My Section\n\nSee [link](#my-section) here.\n",
        )
        .expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Position on the heading "## My Section" (line 2, char 4)
        let result = engine.execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 4), Position::new(2, 4)),
        });

        match result {
            CoreOperationResult::Locations(locations) => {
                assert!(
                    !locations.is_empty(),
                    "expected at least 1 markdown link reference"
                );
            }
            other => panic!("expected Locations result, got: {other:?}"),
        }
    }

    #[test]
    fn find_references_returns_xml_tag_refs_across_documents() {
        let ws = TempWorkspace::new("find-refs-xml");
        let a = ws.root().join("a.md");
        fs::write(
            &a,
            "# Doc A\n\n<agent>content</agent>\n\n<agent>more</agent>\n",
        )
        .expect("a.md should be created");
        let b = ws.root().join("b.md");
        fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Position on first <agent> tag in a.md (line 2, char 1 = within "agent")
        let result = engine.execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
        });

        match result {
            CoreOperationResult::Locations(locations) => {
                // 2 in a.md + 1 in b.md = 3 total references
                assert_eq!(
                    locations.len(),
                    3,
                    "expected 3 XML tag references, got {}",
                    locations.len()
                );
            }
            other => panic!("expected Locations result, got: {other:?}"),
        }
    }

    #[test]
    fn find_references_returns_error_for_unknown_document() {
        let ws = TempWorkspace::new("find-refs-unknown");
        let a = ws.root().join("a.md");
        fs::write(&a, "# Heading\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let unknown = ws.root().join("nonexistent.md");
        let result = engine.execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&unknown),
            position: Range::new(Position::new(0, 2), Position::new(0, 2)),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for unknown document, got: {other:?}"),
        }
    }

    #[test]
    fn find_references_returns_error_for_position_without_symbol() {
        let ws = TempWorkspace::new("find-refs-nosymbol");
        let a = ws.root().join("a.md");
        // Line 0: "# Heading", Line 1: empty, Line 2: "Some text"
        fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Position on "Some text" (line 2) - no heading or XML tag there
        let result = engine.execute(CoreOperation::FindReferences {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 2), Position::new(2, 2)),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected - no symbol at position */ }
            other => panic!("expected error for no-symbol position, got: {other:?}"),
        }
    }

    /// Helper to flatten WorkspaceEdit into a vec of (uri, range, new_text) sorted deterministically.
    fn flatten_workspace_edit(result: CoreOperationResult) -> Vec<(DocumentUri, Range, String)> {
        match result {
            CoreOperationResult::WorkspaceEdit(edits) => {
                let mut flat: Vec<(DocumentUri, Range, String)> = edits
                    .into_iter()
                    .flat_map(|(uri, changes)| {
                        changes
                            .into_iter()
                            .map(move |(range, text)| (uri.clone(), range, text))
                    })
                    .collect();
                flat.sort_by(|(uri_a, range_a, _), (uri_b, range_b, _)| {
                    uri_a
                        .as_str()
                        .cmp(uri_b.as_str())
                        .then_with(|| compare_ranges(*range_a, *range_b))
                });
                flat
            }
            other => panic!("expected WorkspaceEdit, got: {other:?}"),
        }
    }

    #[test]
    fn rename_heading_edits_heading_text_and_wiki_link_and_markdown_anchor() {
        let ws = TempWorkspace::new("rename-heading");
        let a = ws.root().join("a.md");
        // Line 0: "# Title"
        // Line 1: ""
        // Line 2: "## Setup"
        // Line 3: ""
        // Line 4: "See [[#setup]] here."
        // Line 5: ""
        // Line 6: "Also [link](#setup) works."
        fs::write(
            &a,
            "# Title\n\n## Setup\n\nSee [[#setup]] here.\n\nAlso [link](#setup) works.\n",
        )
        .expect("a.md should be created");

        let b = ws.root().join("b.md");
        // Line 0: "# Other"
        // Line 1: ""
        // Line 2: "Check [[a#setup]] link."
        fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n").expect("b.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Rename "## Setup" to "Installation"
        let result = engine.execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 3), Position::new(2, 3)),
            new_name: "Installation".to_string(),
        });

        let edits = flatten_workspace_edit(result);
        // Should have at least 3 edits:
        // 1. Heading text "Setup" -> "Installation" in a.md
        // 2. Wiki link #setup -> Installation in a.md (the raw heading text in [[#setup]])
        // 3. Wiki link #setup -> Installation in b.md (the heading text in [[a#setup]])
        // 4. Markdown link anchor #setup -> #installation in a.md
        assert!(
            edits.len() >= 3,
            "expected at least 3 rename edits, got {}",
            edits.len()
        );

        // Verify at least one edit targets the heading text itself
        let a_uri = DocumentUri::from_file_path(&a);
        let heading_edits: Vec<_> = edits
            .iter()
            .filter(|(uri, _, text)| *uri == a_uri && text == "Installation")
            .collect();
        assert!(
            !heading_edits.is_empty(),
            "should have at least one edit replacing heading text with 'Installation'"
        );
    }

    #[test]
    fn rename_xml_tag_edits_open_and_close_tags_across_documents() {
        let ws = TempWorkspace::new("rename-xml");
        let a = ws.root().join("a.md");
        // Line 0: "# Doc A"
        // Line 1: ""
        // Line 2: "<agent>content</agent>"
        fs::write(&a, "# Doc A\n\n<agent>content</agent>\n").expect("a.md should be created");

        let b = ws.root().join("b.md");
        // Line 0: "# Doc B"
        // Line 1: ""
        // Line 2: "<agent>stuff</agent>"
        fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n").expect("b.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Rename <agent> to <tool> - cursor on first <agent> in a.md
        let result = engine.execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
            new_name: "tool".to_string(),
        });

        let edits = flatten_workspace_edit(result);
        // Each tag has open + close = 2 edits per tag, 2 tags total = 4 edits
        assert_eq!(
            edits.len(),
            4,
            "expected 4 XML rename edits (2 per tag), got {}",
            edits.len()
        );

        // All edits should use the new name "tool"
        for (_, _, new_text) in &edits {
            assert_eq!(new_text, "tool", "all edits should rename to 'tool'");
        }
    }

    #[test]
    fn rename_self_closing_xml_tag_edits_only_open_tag() {
        let ws = TempWorkspace::new("rename-xml-self-close");
        let a = ws.root().join("a.md");
        fs::write(&a, "# Doc\n\n<br/>\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Rename <br/> to <hr/> - cursor on <br/> in a.md
        let result = engine.execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 1), Position::new(2, 1)),
            new_name: "hr".to_string(),
        });

        let edits = flatten_workspace_edit(result);
        // Self-closing: only 1 edit (open tag name only, no close tag)
        assert_eq!(
            edits.len(),
            1,
            "expected 1 edit for self-closing XML tag, got {}",
            edits.len()
        );
        assert_eq!(edits[0].2, "hr");
    }

    #[test]
    fn rename_returns_error_for_unknown_document() {
        let ws = TempWorkspace::new("rename-unknown");
        let a = ws.root().join("a.md");
        fs::write(&a, "# Heading\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let unknown = ws.root().join("nonexistent.md");
        let result = engine.execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&unknown),
            position: Range::new(Position::new(0, 2), Position::new(0, 2)),
            new_name: "NewName".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for unknown document, got: {other:?}"),
        }
    }

    #[test]
    fn rename_returns_error_for_position_without_renameable_symbol() {
        let ws = TempWorkspace::new("rename-nosymbol");
        let a = ws.root().join("a.md");
        fs::write(&a, "# Heading\n\nSome text\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Position on "Some text" (line 2) - no heading or XML tag
        let result = engine.execute(CoreOperation::Rename {
            uri: DocumentUri::from_file_path(&a),
            position: Range::new(Position::new(2, 2), Position::new(2, 2)),
            new_name: "Whatever".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for no-symbol position, got: {other:?}"),
        }
    }

    #[test]
    fn skips_non_utf8_documents_without_failing_startup() {
        let ws = TempWorkspace::new("invalid-utf8");
        let good = ws.root().join("good.md");
        let bad = ws.root().join("bad.md");
        fs::write(&good, "# Intro\n").expect("valid markdown should be created");
        fs::write(&bad, [0xFFu8, 0xFEu8, 0xFDu8]).expect("invalid utf8 markdown should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let outline = engine.execute(CoreOperation::GetOutline {
            uri: DocumentUri::from_file_path(&good),
        });
        match outline {
            CoreOperationResult::Outline(headings) => assert_eq!(headings, vec!["Intro"]),
            other => panic!("expected outline result, got: {other:?}"),
        }
    }
}
