use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Range};
use markymark_index::{DocumentIndex, RealmIndex};
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
            CoreOperation::Rename { .. } => CoreOperationResult::Error(CoreError::NotImplemented(
                "rename is not wired into the MCP runtime yet".to_string(),
            )),
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
        fs::write(&b, "# Other\n\nCheck [[a#setup]] link.\n")
            .expect("b.md should be created");

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
        fs::write(&a, "# Doc A\n\n<agent>content</agent>\n\n<agent>more</agent>\n")
            .expect("a.md should be created");
        let b = ws.root().join("b.md");
        fs::write(&b, "# Doc B\n\n<agent>stuff</agent>\n")
            .expect("b.md should be created");

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
