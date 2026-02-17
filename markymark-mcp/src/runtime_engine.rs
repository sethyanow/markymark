use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{anyhow, bail};
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, RealmIndex, StructuredDocumentIndex};
use markymark_parser::structured::parse_structured;
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
    let documents = collect_documents(root);

    for (path, kind) in documents {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let uri = DocumentUri::from_file_path(&path);

        if kind == DocumentKind::Markdown {
            let ast = match parser.parse(&source) {
                Ok(ast) => ast,
                Err(_) => continue,
            };
            realm.index.add_document(uri, DocumentIndex::from_ast(ast));
        } else {
            let ast = match parse_structured(&source, kind) {
                Ok(ast) => ast,
                Err(_) => continue,
            };
            realm
                .index
                .add_structured_document(uri, StructuredDocumentIndex::from_ast(ast));
        }
    }
}

/// Remove all documents under a root from a realm's index.
fn unindex_root_from_realm(root: &Path, realm: &mut RealmData) {
    let prefix = DocumentUri::from_file_path(root);
    let prefix_str = prefix.as_str();

    // Collect URIs to remove (cannot mutate while iterating).
    let to_remove: Vec<DocumentUri> = realm
        .index
        .iter_all_documents()
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
            // --- Document operations (read from specified realm, falling back to default) ---
            CoreOperation::GetOutline {
                uri,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                let realm = &realm_data.index;
                match realm.get_any_document(&uri) {
                    Some(markymark_index::AnyDocumentIndex::Markdown(index)) => {
                        CoreOperationResult::Outline(
                            index
                                .headings()
                                .iter()
                                .map(|heading| heading.text.to_string())
                                .collect(),
                        )
                    }
                    Some(markymark_index::AnyDocumentIndex::Structured(index)) => {
                        CoreOperationResult::Outline(
                            index
                                .keys()
                                .iter()
                                .map(|k| {
                                    let indent = "  ".repeat(k.depth);
                                    format!("{indent}{}: {:?}", k.path, k.value_kind)
                                })
                                .collect(),
                        )
                    }
                    None => CoreOperationResult::Error(CoreError::Message(format!(
                        "document is not indexed: {}",
                        uri.as_str()
                    ))),
                }
            }
            CoreOperation::SearchSymbols {
                query,
                realm: realm_name,
            } => {
                let query = query.trim().to_string();
                if query.is_empty() {
                    return CoreOperationResult::Error(CoreError::Message(
                        "search query cannot be empty".to_string(),
                    ));
                }

                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                let realm = &realm_data.index;
                let mut matches = Vec::new();
                let query_lower = query.to_lowercase();

                // Search markdown headings
                for (uri, index) in realm.iter_documents() {
                    for heading in index.headings() {
                        if heading.text.to_lowercase().contains(&query_lower) {
                            matches.push((heading.text.to_string(), uri.clone(), heading.range));
                        }
                    }
                }

                // Search structured document key paths
                for (uri, path, _key, _kind, range) in realm.search_key_paths(&query) {
                    matches.push((path, uri, range));
                }

                matches.sort_by(|(name_a, uri_a, range_a), (name_b, uri_b, range_b)| {
                    name_a
                        .cmp(name_b)
                        .then_with(|| uri_a.as_str().cmp(uri_b.as_str()))
                        .then_with(|| compare_ranges(*range_a, *range_b))
                });

                CoreOperationResult::Symbols(matches)
            }
            CoreOperation::FindReferences {
                uri,
                position,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                let realm = &realm_data.index;
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
                            if wl.heading == Some(slug) {
                                locations.push((doc_uri.clone(), wl.range));
                            }
                        }
                        for ml in doc_index.markdown_links() {
                            if ml.anchor == Some(slug) {
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
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                let realm = &realm_data.index;
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
                    return rename_xml_tag(realm, xml_tag.tag_name, &new_name);
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
                    structured_doc_count: realm_data.index.structured_count(),
                    key_path_count: realm_data.index.key_path_count(),
                }
            }
            CoreOperation::DependencyGraph { realm, format } => {
                let state = self.state.read().expect("lock poisoned");
                let realm_data = match state.get(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };

                let content = build_dependency_graph(&realm_data.index, &format);
                match content {
                    Ok(content) => CoreOperationResult::DependencyGraph {
                        realm,
                        format,
                        content,
                    },
                    Err(msg) => CoreOperationResult::Error(CoreError::Message(msg)),
                }
            }
            CoreOperation::ExportIndex {
                uri,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                let realm = &realm_data.index;
                match realm.get_any_document(&uri) {
                    Some(markymark_index::AnyDocumentIndex::Markdown(index)) => {
                        let headings = index
                            .headings()
                            .iter()
                            .map(|h| (h.text.to_string(), h.level, h.range))
                            .collect();

                        let xml_tags = index
                            .xml_tags()
                            .iter()
                            .map(|x| (x.tag_name.to_string(), x.range))
                            .collect();

                        let wiki_links = index
                            .wiki_links()
                            .iter()
                            .map(|wl| {
                                (
                                    wl.target.to_string(),
                                    wl.heading.map(|h| h.to_string()),
                                    wl.range,
                                )
                            })
                            .collect();

                        let markdown_links = index
                            .markdown_links()
                            .iter()
                            .map(|ml| (ml.text.to_string(), ml.url.to_string(), ml.range))
                            .collect();

                        CoreOperationResult::DocumentExport {
                            uri,
                            document_kind: Some(DocumentKind::Markdown),
                            headings,
                            xml_tags,
                            wiki_links,
                            markdown_links,
                        }
                    }
                    Some(markymark_index::AnyDocumentIndex::Structured(index)) => {
                        // For structured docs, export key paths as "headings"
                        // with depth as level and key range as position.
                        let headings = index
                            .keys()
                            .iter()
                            .map(|k| (k.path.clone(), (k.depth as u8) + 1, k.key_range))
                            .collect();

                        CoreOperationResult::DocumentExport {
                            uri,
                            document_kind: Some(index.kind()),
                            headings,
                            xml_tags: Vec::new(),
                            wiki_links: Vec::new(),
                            markdown_links: Vec::new(),
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

fn collect_documents(root: &Path) -> Vec<(PathBuf, DocumentKind)> {
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

            if let Some(kind) = DocumentKind::from_path(&path) {
                files.push((path, kind));
            }
        }
    }

    files.sort_by(|(a, _), (b, _)| a.cmp(b));
    files
}

/// Build a dependency graph from the realm's indexed documents.
///
/// Returns the graph as either JSON or DOT format.
fn build_dependency_graph(realm: &RealmIndex, format: &str) -> Result<String, String> {
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    // Collect document URIs and outgoing links.
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut edges: Vec<(String, String, String)> = Vec::new();

    for (uri, index) in realm.iter_documents() {
        let from = uri.as_str().to_string();
        nodes.insert(from.clone());

        for wl in index.wiki_links() {
            // Wiki links target page names, not full URIs.
            // Record the target as-is for the graph.
            let to = format!("wiki:{}", wl.target);
            nodes.insert(to.clone());
            edges.push((from.clone(), to, "wiki_link".to_string()));
        }

        for ml in index.markdown_links() {
            if ml.url.starts_with("http://") || ml.url.starts_with("https://") {
                continue; // Skip external URLs.
            }
            let to = ml.url.to_string();
            nodes.insert(to.clone());
            edges.push((from.clone(), to, "markdown_link".to_string()));
        }
    }

    match format {
        "json" => {
            let nodes_json: Vec<_> = nodes.iter().map(|n| json!({ "id": n })).collect();
            let edges_json: Vec<_> = edges
                .iter()
                .map(|(from, to, kind)| json!({ "from": from, "to": to, "kind": kind }))
                .collect();
            let graph = json!({ "nodes": nodes_json, "edges": edges_json });
            serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())
        }
        "dot" => {
            let mut out = String::from("digraph dependency_graph {\n");
            // Assign short labels for readability.
            let label_map: BTreeMap<&str, usize> = nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.as_str(), i))
                .collect();
            for (name, idx) in &label_map {
                // Use the file stem or short name as label.
                let label = name.rsplit('/').next().unwrap_or(name);
                out.push_str(&format!("  n{idx} [label={label:?}];\n"));
            }
            for (from, to, kind) in &edges {
                let from_idx = label_map[from.as_str()];
                let to_idx = label_map[to.as_str()];
                out.push_str(&format!("  n{from_idx} -> n{to_idx} [label={kind:?}];\n"));
            }
            out.push_str("}\n");
            Ok(out)
        }
        other => Err(format!("unsupported dependency graph format: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::Position;
    use std::fs;

    fn make_temp_realm_dir(suffix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("marky-realm-{}-{}", suffix, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_engine_with_custom_realm(realm_name: &str, dir: &Path) -> RuntimeEngine {
        let engine = RuntimeEngine::default();
        // create the realm
        engine.execute(CoreOperation::CreateRealm {
            name: realm_name.to_string(),
        });
        // index the directory into it
        engine.execute(CoreOperation::AddRoot {
            realm: realm_name.to_string(),
            root: dir.to_path_buf(),
        });
        engine
    }

    #[test]
    fn get_outline_uses_named_realm() {
        let dir = make_temp_realm_dir("get-outline");
        fs::write(dir.join("doc.md"), "# Hello World\n\n## Section\n").unwrap();
        let engine = make_engine_with_custom_realm("my-realm", &dir);

        let uri_str = format!("file://{}", dir.join("doc.md").display());
        let uri = DocumentUri::new(&uri_str).unwrap();

        // Should fail without realm (default realm has no such doc)
        let result = engine.execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: None,
        });
        assert!(
            matches!(result, CoreOperationResult::Error(_)),
            "expected error when querying default realm, got {result:?}"
        );

        // Should succeed with the correct realm
        let result = engine.execute(CoreOperation::GetOutline {
            uri: uri.clone(),
            realm: Some("my-realm".to_string()),
        });
        assert!(
            matches!(result, CoreOperationResult::Outline(_)),
            "expected Outline from named realm, got {result:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_index_uses_named_realm() {
        let dir = make_temp_realm_dir("export-index");
        fs::write(dir.join("doc.md"), "# Title\n").unwrap();
        let engine = make_engine_with_custom_realm("export-realm", &dir);

        let uri_str = format!("file://{}", dir.join("doc.md").display());
        let uri = DocumentUri::new(&uri_str).unwrap();

        let result = engine.execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: Some("export-realm".to_string()),
        });
        assert!(
            matches!(result, CoreOperationResult::DocumentExport { .. }),
            "expected DocumentExport from named realm, got {result:?}"
        );

        let result_default = engine.execute(CoreOperation::ExportIndex { uri, realm: None });
        assert!(
            matches!(result_default, CoreOperationResult::Error(_)),
            "expected error from default realm, got {result_default:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_symbols_uses_named_realm() {
        let dir = make_temp_realm_dir("search-symbols");
        fs::write(dir.join("doc.md"), "# UniqueHeadingXYZ\n").unwrap();
        let engine = make_engine_with_custom_realm("search-realm", &dir);

        // Default realm should return no matches for the unique heading
        let result = engine.execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: None,
        });
        if let CoreOperationResult::Symbols(matches) = result {
            assert!(
                matches.is_empty(),
                "default realm should not have the heading"
            );
        } else {
            panic!("expected Symbols result");
        }

        // Named realm should find it
        let result = engine.execute(CoreOperation::SearchSymbols {
            query: "UniqueHeadingXYZ".to_string(),
            realm: Some("search-realm".to_string()),
        });
        if let CoreOperationResult::Symbols(matches) = result {
            assert!(!matches.is_empty(), "named realm should have the heading");
        } else {
            panic!("expected Symbols result");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_references_uses_named_realm() {
        let dir = make_temp_realm_dir("find-refs");
        // A heading with a wiki-link reference in the same file
        fs::write(dir.join("doc.md"), "# My Heading\n\n[[My Heading]]\n").unwrap();
        let engine = make_engine_with_custom_realm("refs-realm", &dir);

        let uri_str = format!("file://{}", dir.join("doc.md").display());
        let uri = DocumentUri::new(&uri_str).unwrap();

        let position = markymark_core::Range {
            start: Position {
                line: 0,
                character: 2,
            },
            end: Position {
                line: 0,
                character: 12,
            },
        };

        // Default realm has no such doc
        let result = engine.execute(CoreOperation::FindReferences {
            uri: uri.clone(),
            position,
            realm: None,
        });
        assert!(
            matches!(result, CoreOperationResult::Error(_)),
            "expected error from default realm, got {result:?}"
        );

        // Named realm should find the references
        let result = engine.execute(CoreOperation::FindReferences {
            uri,
            position,
            realm: Some("refs-realm".to_string()),
        });
        assert!(
            !matches!(result, CoreOperationResult::Error(_)),
            "expected success from named realm, got {result:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_uses_named_realm() {
        let dir = make_temp_realm_dir("rename");
        fs::write(dir.join("doc.md"), "# Old Name\n").unwrap();
        let engine = make_engine_with_custom_realm("rename-realm", &dir);

        let uri_str = format!("file://{}", dir.join("doc.md").display());
        let uri = DocumentUri::new(&uri_str).unwrap();

        let position = markymark_core::Range {
            start: Position {
                line: 0,
                character: 2,
            },
            end: Position {
                line: 0,
                character: 10,
            },
        };

        // Default realm has no such doc
        let result = engine.execute(CoreOperation::Rename {
            uri: uri.clone(),
            position,
            new_name: "New Name".to_string(),
            realm: None,
        });
        assert!(
            matches!(result, CoreOperationResult::Error(_)),
            "expected error from default realm, got {result:?}"
        );

        // Named realm should work
        let result = engine.execute(CoreOperation::Rename {
            uri,
            position,
            new_name: "New Name".to_string(),
            realm: Some("rename-realm".to_string()),
        });
        assert!(
            !matches!(result, CoreOperationResult::Error(_)),
            "expected success from named realm, got {result:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_documents_includes_json_alongside_markdown() {
        let dir = std::env::temp_dir().join(format!("marky-collect-mixed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# Hello\n").unwrap();
        fs::write(dir.join("config.json"), "{}").unwrap();
        fs::write(dir.join("settings.yaml"), "key: val\n").unwrap();
        fs::write(dir.join("main.rs"), "fn main() {}").unwrap();

        let docs = collect_documents(&dir);
        let kinds: Vec<_> = docs.iter().map(|(_, k)| *k).collect();

        assert!(kinds.contains(&DocumentKind::Markdown));
        assert!(kinds.contains(&DocumentKind::Json));
        assert!(kinds.contains(&DocumentKind::Yaml));
        // main.rs should NOT be collected
        assert_eq!(docs.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_documents_markdown_unchanged() {
        let dir = std::env::temp_dir().join(format!("marky-collect-md-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("readme.md"), "# R\n").unwrap();
        fs::write(dir.join("guide.markdown"), "# G\n").unwrap();

        let docs = collect_documents(&dir);
        assert_eq!(docs.len(), 2);
        assert!(docs.iter().all(|(_, k)| *k == DocumentKind::Markdown));

        let _ = fs::remove_dir_all(&dir);
    }
}
