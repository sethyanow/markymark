use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{anyhow, bail};
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, RealmIndex};
use markymark_parser::Parser;

use crate::rename_ops::{compare_ranges, rename_heading, rename_xml_tag};

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

            // --- Query operations ---
            CoreOperation::RealmStats { realm } => {
                let state = self.state.read().expect("lock poisoned");
                let realm_data = match state.get(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };

                let mut heading_count = 0;
                let mut xml_tag_count = 0;
                let mut wiki_link_count = 0;
                let mut markdown_link_count = 0;

                for (_uri, index) in realm_data.index.iter_documents() {
                    heading_count += index.headings().len();
                    xml_tag_count += index.xml_tags().len();
                    wiki_link_count += index.wiki_links().len();
                    markdown_link_count += index.markdown_links().len();
                }

                CoreOperationResult::RealmStats {
                    name: realm,
                    root_count: realm_data.roots.len(),
                    document_count: realm_data.index.document_count(),
                    heading_count,
                    xml_tag_count,
                    wiki_link_count,
                    markdown_link_count,
                }
            }
            CoreOperation::ExportIndex { uri } => {
                let state = self.state.read().expect("lock poisoned");
                let realm = &state[DEFAULT_REALM].index;
                match realm.get_document(&uri) {
                    Some(index) => {
                        let headings = index
                            .headings()
                            .iter()
                            .map(|h| (h.text.clone(), h.level, h.range))
                            .collect();

                        let xml_tags = index
                            .xml_tags()
                            .iter()
                            .map(|x| (x.tag_name.clone(), x.range))
                            .collect();

                        let wiki_links = index
                            .wiki_links()
                            .iter()
                            .map(|wl| (wl.target.clone(), wl.heading.clone(), wl.range))
                            .collect();

                        let markdown_links = index
                            .markdown_links()
                            .iter()
                            .map(|ml| (ml.text.clone(), ml.url.clone(), ml.range))
                            .collect();

                        CoreOperationResult::DocumentExport {
                            uri,
                            headings,
                            xml_tags,
                            wiki_links,
                            markdown_links,
                        }
                    }
                    None => CoreOperationResult::Error(CoreError::Message(format!(
                        "document is not indexed: {}",
                        uri.as_str()
                    ))),
                }
            }
        }
    }
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
