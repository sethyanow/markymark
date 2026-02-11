use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{anyhow, bail};
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri, Position, Range};
use markymark_index::{slugify, DocumentIndex, MarkdownLinkEntry, RealmIndex, WikiLinkEntry};
use markymark_parser::Parser;

/// The name of the default realm created at startup.
const DEFAULT_REALM: &str = "default";

/// Per-realm state: index plus tracked workspace roots.
struct RealmData {
    index: RealmIndex,
    roots: Vec<PathBuf>,
}

impl RealmData {
    fn new() -> Self {
        Self {
            index: RealmIndex::new(),
            roots: Vec::new(),
        }
    }
}

/// Production core engine backed by named realms of indexed markdown workspaces.
///
/// A "default" realm is always created at startup with the initial workspace roots.
/// Additional realms can be created/destroyed dynamically via [`CoreOperation`].
pub struct RuntimeEngine {
    state: RwLock<HashMap<String, RealmData>>,
}

impl Default for RuntimeEngine {
    fn default() -> Self {
        let mut realms = HashMap::new();
        realms.insert(DEFAULT_REALM.to_string(), RealmData::new());
        Self {
            state: RwLock::new(realms),
        }
    }
}

impl RuntimeEngine {
    /// Build a runtime engine from workspace roots.
    ///
    /// All markdown files (`*.md`, `*.markdown`) under the provided roots are indexed
    /// into the "default" realm.
    /// Invalid roots fail startup. Individual document read/parse failures are skipped.
    pub fn from_workspace_roots(workspace_roots: Vec<PathBuf>) -> anyhow::Result<Self> {
        if workspace_roots.is_empty() {
            bail!("at least one workspace root is required");
        }

        let mut parser = Parser::new().map_err(|err| anyhow!(err.to_string()))?;
        let mut default_realm = RealmData::new();

        for root in workspace_roots {
            validate_workspace_root(&root)?;
            index_root_into_realm(&mut parser, &root, &mut default_realm);
            default_realm.roots.push(root);
        }

        let mut realms = HashMap::new();
        realms.insert(DEFAULT_REALM.to_string(), default_realm);

        Ok(Self {
            state: RwLock::new(realms),
        })
    }
}

/// Read the source text for a document from disk.
fn read_document_text(uri: &DocumentUri) -> Option<String> {
    let path = uri.to_file_path()?;
    fs::read_to_string(path).ok()
}

/// Rename a heading and all references to it across a realm.
fn rename_heading(
    realm: &RealmIndex,
    uri: &DocumentUri,
    heading: markymark_index::HeadingEntry,
    new_name: &str,
) -> CoreOperationResult {
    let old_slug = heading.slug.clone();
    let new_slug = slugify(new_name);
    let mut doc_edits: HashMap<DocumentUri, Vec<(Range, String)>> = HashMap::new();

    // 1. Edit the heading text itself.
    //    Skip the "## " prefix to find the text-only range.
    if let Some(text) = read_document_text(uri) {
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
    for (doc_uri, doc_index) in realm.iter_documents() {
        let doc_text = read_document_text(doc_uri);

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

    build_workspace_edit(doc_edits)
}

/// Rename an XML tag across all documents in a realm.
fn rename_xml_tag(realm: &RealmIndex, old_name: &str, new_name: &str) -> CoreOperationResult {
    let mut doc_edits: HashMap<DocumentUri, Vec<(Range, String)>> = HashMap::new();

    for (doc_uri, doc_index) in realm.iter_documents() {
        for xml in doc_index.xml_tags() {
            if xml.tag_name == old_name {
                // Opening tag name: starts after '<'
                let name_start = Position::new(xml.range.start.line, xml.range.start.character + 1);
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

    build_workspace_edit(doc_edits)
}

/// Sort and build a deterministic WorkspaceEdit from collected document edits.
fn build_workspace_edit(
    doc_edits: HashMap<DocumentUri, Vec<(Range, String)>>,
) -> CoreOperationResult {
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

/// Index all markdown files under a root into a realm.
fn index_root_into_realm(parser: &mut Parser, root: &Path, realm: &mut RealmData) {
    let markdown_files = collect_markdown_files(root);

    for path in markdown_files {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let ast = match parser.parse(&source) {
            Ok(ast) => ast,
            Err(_) => continue,
        };

        realm.index.add_document(
            DocumentUri::from_file_path(&path),
            DocumentIndex::from_ast(&ast),
        );
    }
}

/// Remove all documents under a root from a realm's index.
fn unindex_root_from_realm(root: &Path, realm: &mut RealmData) {
    let prefix = DocumentUri::from_file_path(root);
    let prefix_str = prefix.as_str();

    // Collect URIs to remove (cannot mutate while iterating).
    let to_remove: Vec<DocumentUri> = realm
        .index
        .iter_documents()
        .filter(|(uri, _)| uri.as_str().starts_with(prefix_str))
        .map(|(uri, _)| uri.clone())
        .collect();

    for uri in to_remove {
        realm.index.remove_document(&uri);
    }
}

impl CoreEngine for RuntimeEngine {
    fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            // --- Document operations (read from default realm) ---
            CoreOperation::GetOutline { uri } => {
                let state = self.state.read().expect("lock poisoned");
                let realm = &state[DEFAULT_REALM].index;
                match realm.get_document(&uri) {
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
                }
            }
            CoreOperation::SearchSymbols { query } => {
                let query = query.trim().to_string();
                if query.is_empty() {
                    return CoreOperationResult::Error(CoreError::Message(
                        "search query cannot be empty".to_string(),
                    ));
                }

                let state = self.state.read().expect("lock poisoned");
                let realm = &state[DEFAULT_REALM].index;
                let mut matches = Vec::new();
                let query_lower = query.to_lowercase();

                for (uri, index) in realm.iter_documents() {
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
                let state = self.state.read().expect("lock poisoned");
                let realm = &state[DEFAULT_REALM].index;
                let index = match realm.get_document(&uri) {
                    Some(idx) => idx,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "document is not indexed: {}",
                            uri.as_str()
                        )));
                    }
                };

                let cursor = position.start;

                if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                    let slug = &heading.slug;
                    let mut locations = Vec::new();

                    for (doc_uri, doc_index) in realm.iter_documents() {
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

                    for (doc_uri, doc_index) in realm.iter_documents() {
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
                let state = self.state.read().expect("lock poisoned");
                let realm = &state[DEFAULT_REALM].index;
                let index = match realm.get_document(&uri) {
                    Some(idx) => idx,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "document is not indexed: {}",
                            uri.as_str()
                        )));
                    }
                };

                let cursor = position.start;

                if let Some(heading) = index.headings().iter().find(|h| h.range.contains(cursor)) {
                    return rename_heading(realm, &uri, heading.clone(), &new_name);
                }

                if let Some(xml_tag) = index.xml_tags().iter().find(|x| x.range.contains(cursor)) {
                    return rename_xml_tag(realm, &xml_tag.tag_name, &new_name);
                }

                CoreOperationResult::Error(CoreError::Message(
                    "no renameable symbol at position".to_string(),
                ))
            }

            // --- Realm management operations ---
            CoreOperation::CreateRealm { name } => {
                if name.is_empty() {
                    return CoreOperationResult::Error(CoreError::Message(
                        "realm name must not be empty".to_string(),
                    ));
                }

                let mut state = self.state.write().expect("lock poisoned");
                if state.contains_key(&name) {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm already exists: {name}"
                    )));
                }

                state.insert(name.clone(), RealmData::new());
                CoreOperationResult::RealmInfo {
                    name,
                    root_count: 0,
                    document_count: 0,
                }
            }
            CoreOperation::DestroyRealm { name } => {
                if name == DEFAULT_REALM {
                    return CoreOperationResult::Error(CoreError::Message(
                        "cannot destroy the default realm".to_string(),
                    ));
                }

                let mut state = self.state.write().expect("lock poisoned");
                if state.remove(&name).is_none() {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {name}"
                    )));
                }

                CoreOperationResult::Ok
            }
            CoreOperation::AddRoot { realm, root } => {
                if let Err(msg) = validate_workspace_root(&root) {
                    return CoreOperationResult::Error(CoreError::Message(msg.to_string()));
                }

                let mut state = self.state.write().expect("lock poisoned");
                let realm_data = match state.get_mut(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };

                // Check for duplicate root (canonicalize for comparison).
                let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
                for existing in &realm_data.roots {
                    let existing_canonical =
                        existing.canonicalize().unwrap_or_else(|_| existing.clone());
                    if canonical == existing_canonical {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "root already added to realm: {}",
                            root.display()
                        )));
                    }
                }

                let mut parser = match Parser::new() {
                    Ok(p) => p,
                    Err(err) => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "failed to create parser: {err}"
                        )));
                    }
                };

                index_root_into_realm(&mut parser, &root, realm_data);
                realm_data.roots.push(root);

                CoreOperationResult::RealmInfo {
                    name: realm.clone(),
                    root_count: realm_data.roots.len(),
                    document_count: realm_data.index.document_count(),
                }
            }
            CoreOperation::RemoveRoot { realm, root } => {
                let mut state = self.state.write().expect("lock poisoned");
                let realm_data = match state.get_mut(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };

                let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
                let pos = realm_data.roots.iter().position(|existing| {
                    existing.canonicalize().unwrap_or_else(|_| existing.clone()) == canonical
                });

                match pos {
                    Some(idx) => {
                        let removed = realm_data.roots.remove(idx);
                        unindex_root_from_realm(&removed, realm_data);

                        CoreOperationResult::RealmInfo {
                            name: realm.clone(),
                            root_count: realm_data.roots.len(),
                            document_count: realm_data.index.document_count(),
                        }
                    }
                    None => CoreOperationResult::Error(CoreError::Message(format!(
                        "root is not tracked in realm: {}",
                        root.display()
                    ))),
                }
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

    // === Realm Management Tests ===

    #[test]
    fn create_realm_returns_realm_info() {
        let ws = TempWorkspace::new("create-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::CreateRealm {
            name: "test-realm".to_string(),
        });

        match result {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => {
                assert_eq!(name, "test-realm");
                assert_eq!(root_count, 0);
                assert_eq!(document_count, 0);
            }
            other => panic!("expected RealmInfo, got: {other:?}"),
        }
    }

    #[test]
    fn create_realm_rejects_duplicate_name() {
        let ws = TempWorkspace::new("create-dup-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Create first realm
        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "my-realm".to_string(),
        });

        // Try duplicate
        let result = engine.execute(CoreOperation::CreateRealm {
            name: "my-realm".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for duplicate realm, got: {other:?}"),
        }
    }

    #[test]
    fn create_realm_rejects_empty_name() {
        let ws = TempWorkspace::new("create-empty-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::CreateRealm {
            name: "".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for empty realm name, got: {other:?}"),
        }
    }

    #[test]
    fn destroy_realm_removes_realm() {
        let ws = TempWorkspace::new("destroy-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Create and then destroy
        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "temp-realm".to_string(),
        });
        let result = engine.execute(CoreOperation::DestroyRealm {
            name: "temp-realm".to_string(),
        });

        match result {
            CoreOperationResult::Ok => { /* expected */ }
            other => panic!("expected Ok for destroy, got: {other:?}"),
        }

        // Creating again should succeed (proves it was destroyed)
        let result = engine.execute(CoreOperation::CreateRealm {
            name: "temp-realm".to_string(),
        });
        match result {
            CoreOperationResult::RealmInfo { name, .. } => {
                assert_eq!(name, "temp-realm");
            }
            other => panic!("expected RealmInfo after re-create, got: {other:?}"),
        }
    }

    #[test]
    fn destroy_realm_rejects_unknown_name() {
        let ws = TempWorkspace::new("destroy-unknown-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::DestroyRealm {
            name: "nonexistent".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for unknown realm, got: {other:?}"),
        }
    }

    #[test]
    fn destroy_realm_rejects_default_realm() {
        let ws = TempWorkspace::new("destroy-default-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::DestroyRealm {
            name: "default".to_string(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected: cannot destroy default realm */ }
            other => panic!("expected error for destroying default realm, got: {other:?}"),
        }
    }

    #[test]
    fn add_root_indexes_markdown_files_in_realm() {
        let ws = TempWorkspace::new("add-root");
        let sub = ws.root().join("docs");
        fs::create_dir_all(&sub).expect("subdirectory should be created");
        fs::write(sub.join("a.md"), "# Alpha\n").expect("a.md should be created");
        fs::write(sub.join("b.md"), "# Beta\n").expect("b.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        // Create a realm and add the docs subdirectory
        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "docs-realm".to_string(),
        });

        let result = engine.execute(CoreOperation::AddRoot {
            realm: "docs-realm".to_string(),
            root: sub,
        });

        match result {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => {
                assert_eq!(name, "docs-realm");
                assert_eq!(root_count, 1);
                assert_eq!(document_count, 2);
            }
            other => panic!("expected RealmInfo, got: {other:?}"),
        }
    }

    #[test]
    fn add_root_rejects_unknown_realm() {
        let ws = TempWorkspace::new("add-root-unknown");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::AddRoot {
            realm: "nonexistent".to_string(),
            root: ws.root(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for unknown realm, got: {other:?}"),
        }
    }

    #[test]
    fn add_root_rejects_invalid_path() {
        let ws = TempWorkspace::new("add-root-invalid");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        });

        let result = engine.execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: PathBuf::from("/nonexistent/path/to/nowhere"),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for invalid path, got: {other:?}"),
        }
    }

    #[test]
    fn add_root_rejects_duplicate_root() {
        let ws = TempWorkspace::new("add-root-dup");
        fs::write(ws.root().join("a.md"), "# A\n").expect("a.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        });
        let _ = engine.execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: ws.root(),
        });

        // Adding same root again should error
        let result = engine.execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: ws.root(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for duplicate root, got: {other:?}"),
        }
    }

    #[test]
    fn remove_root_unindexes_documents() {
        let ws = TempWorkspace::new("remove-root");
        let docs = ws.root().join("docs");
        fs::create_dir_all(&docs).expect("docs dir should be created");
        fs::write(docs.join("x.md"), "# X\n").expect("x.md should be created");

        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        });
        let _ = engine.execute(CoreOperation::AddRoot {
            realm: "r".to_string(),
            root: docs.clone(),
        });

        let result = engine.execute(CoreOperation::RemoveRoot {
            realm: "r".to_string(),
            root: docs,
        });

        match result {
            CoreOperationResult::RealmInfo {
                name,
                root_count,
                document_count,
            } => {
                assert_eq!(name, "r");
                assert_eq!(root_count, 0);
                assert_eq!(document_count, 0);
            }
            other => panic!("expected RealmInfo after remove, got: {other:?}"),
        }
    }

    #[test]
    fn remove_root_rejects_unknown_realm() {
        let ws = TempWorkspace::new("remove-root-unknown-realm");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let result = engine.execute(CoreOperation::RemoveRoot {
            realm: "nonexistent".to_string(),
            root: ws.root(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for unknown realm, got: {other:?}"),
        }
    }

    #[test]
    fn remove_root_rejects_untracked_root() {
        let ws = TempWorkspace::new("remove-root-untracked");
        let engine =
            RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");

        let _ = engine.execute(CoreOperation::CreateRealm {
            name: "r".to_string(),
        });

        // Try to remove a root that was never added
        let result = engine.execute(CoreOperation::RemoveRoot {
            realm: "r".to_string(),
            root: ws.root(),
        });

        match result {
            CoreOperationResult::Error(_) => { /* expected */ }
            other => panic!("expected error for untracked root, got: {other:?}"),
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
