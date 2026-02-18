use std::collections::HashMap;
use std::fs;
#[cfg(feature = "semantic-search")]
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
#[cfg(feature = "semantic-search")]
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{anyhow, bail};
#[cfg(feature = "semantic-search")]
use markymark_core::engine::SemanticSearchMatch;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
#[cfg(feature = "semantic-search")]
use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri, Range};
use markymark_index::{DocumentIndex, RealmIndex, StructuredDocumentIndex};
use markymark_kernels::fuzzy_match;
use markymark_kernels::tokens;
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
            index: build_realm_index(),
            roots: Vec::new(),
        }
    }
}

fn build_realm_index() -> RealmIndex {
    #[cfg(feature = "semantic-search")]
    {
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(HashEmbeddingProvider::new(128));
        RealmIndex::new_with_embeddings(provider).unwrap_or_else(|err| {
            eprintln!("warning: failed to initialize semantic index; falling back to plain realm index: {err}");
            RealmIndex::new()
        })
    }

    #[cfg(not(feature = "semantic-search"))]
    {
        RealmIndex::new()
    }
}

#[cfg(feature = "semantic-search")]
#[derive(Debug, Clone)]
struct HashEmbeddingProvider {
    dims: u32,
}

#[cfg(feature = "semantic-search")]
impl HashEmbeddingProvider {
    fn new(dims: u32) -> Self {
        Self { dims }
    }
}

#[cfg(feature = "semantic-search")]
impl EmbeddingProvider for HashEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        if self.dims == 0 {
            return Err(EmbedError::InvalidInput(
                "embedding dimensions must be > 0".to_string(),
            ));
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(EmbedError::InvalidInput(
                "semantic query must not be empty".to_string(),
            ));
        }

        let mut out = vec![0.0_f32; self.dims as usize];
        for token in trimmed
            .split(|c: char| !c.is_alphanumeric())
            .filter(|token| !token.is_empty())
        {
            let norm = token.to_ascii_lowercase();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            norm.hash(&mut hasher);
            let idx = (hasher.finish() as usize) % out.len();
            out[idx] += 1.0;
        }

        let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut out {
                *v /= norm;
            }
        }
        Ok(out)
    }

    fn dimensions(&self) -> u32 {
        self.dims
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
                let mut scored_matches: Vec<(i32, bool, String, DocumentUri, Range)> = Vec::new();

                // Search markdown headings with fuzzy ranking.
                for (uri, index) in realm.iter_documents() {
                    for heading in index.headings() {
                        if let Ok(m) = fuzzy_match(&query, heading.text) {
                            if m.score > 0 {
                                scored_matches.push((
                                    m.score,
                                    m.starts_with,
                                    heading.text.to_string(),
                                    uri.clone(),
                                    heading.range,
                                ));
                            }
                        }
                    }
                }

                // Search structured document key paths with fuzzy ranking.
                for (uri, path, _key, _kind, range) in realm.search_key_paths(&query) {
                    if let Ok(m) = fuzzy_match(&query, &path) {
                        if m.score > 0 {
                            scored_matches.push((m.score, m.starts_with, path, uri, range));
                        }
                    }
                }

                scored_matches.sort_by(
                    |(score_a, starts_a, name_a, uri_a, range_a),
                     (score_b, starts_b, name_b, uri_b, range_b)| {
                        score_b
                            .cmp(score_a)
                            .then_with(|| starts_b.cmp(starts_a))
                            .then_with(|| name_a.cmp(name_b))
                            .then_with(|| uri_a.as_str().cmp(uri_b.as_str()))
                            .then_with(|| compare_ranges(*range_a, *range_b))
                    },
                );

                let matches = scored_matches
                    .into_iter()
                    .map(|(_, _, name, uri, range)| (name, uri, range))
                    .collect();

                CoreOperationResult::Symbols(matches)
            }
            CoreOperation::SemanticSearch {
                query,
                realm,
                top_k,
                min_score,
            } => {
                let query = query.trim().to_string();
                if query.is_empty() {
                    return CoreOperationResult::Error(CoreError::Message(
                        "semantic query cannot be empty".to_string(),
                    ));
                }

                let realm_name = realm.unwrap_or_else(|| DEFAULT_REALM.to_string());
                let state = self.state.read().expect("lock poisoned");
                let realm_data = match state.get(&realm_name) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm_name}"
                        )));
                    }
                };

                #[cfg(not(feature = "semantic-search"))]
                {
                    let _ = (realm_data, top_k, min_score);
                    CoreOperationResult::Error(CoreError::NotImplemented(
                        "semantic-search feature is not enabled for markymark-mcp".to_string(),
                    ))
                }

                #[cfg(feature = "semantic-search")]
                {
                    let results = match realm_data.index.semantic_search(
                        &query,
                        top_k,
                        min_score.clamp(0.0, 1.0),
                    ) {
                        Ok(results) => results,
                        Err(err) => {
                            return CoreOperationResult::Error(CoreError::Message(format!(
                                "semantic search failed: {err}"
                            )));
                        }
                    };

                    CoreOperationResult::SemanticMatches(
                        results
                            .into_iter()
                            .map(|result| {
                                let section_preview = preview_for_range(
                                    &result.doc_uri,
                                    result.section_range,
                                    &result.heading,
                                );
                                SemanticSearchMatch {
                                    doc_uri: result.doc_uri,
                                    heading: result.heading,
                                    heading_level: result.heading_level,
                                    score: result.score,
                                    section_range: result.section_range,
                                    section_preview,
                                }
                            })
                            .collect(),
                    )
                }
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
            CoreOperation::RealmStats {
                realm,
                check_duplicates,
                include_token_counts,
            } => {
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

                let duplicate_pairs = if check_duplicates {
                    #[cfg(feature = "semantic-search")]
                    {
                        Some(realm_data.index.detect_semantic_duplicates(0.85).len())
                    }
                    #[cfg(not(feature = "semantic-search"))]
                    {
                        None
                    }
                } else {
                    None
                };

                let total_tokens = if include_token_counts {
                    let (total, unreadable_docs) = total_tokens_for_realm(&realm_data.index);
                    if unreadable_docs > 0 {
                        eprintln!(
                            "warning: token count omitted for realm '{realm}' due to {unreadable_docs} unreadable documents"
                        );
                        None
                    } else {
                        Some(total)
                    }
                } else {
                    None
                };

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
                    duplicate_pairs,
                    total_tokens,
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

                        use markymark_index::document::{
                            FrontmatterValueEntry, PropertyValueEntry,
                        };
                        let frontmatter = index
                            .frontmatter()
                            .iter()
                            .map(|e| {
                                let values = match &e.value {
                                    FrontmatterValueEntry::String(s) => vec![s.to_string()],
                                    FrontmatterValueEntry::List(items) => {
                                        items.iter().map(|s| s.to_string()).collect()
                                    }
                                };
                                (e.key.to_string(), values)
                            })
                            .collect();

                        let properties = index
                            .properties()
                            .iter()
                            .map(|e| {
                                let value = match &e.value {
                                    PropertyValueEntry::String(s) => s.to_string(),
                                    PropertyValueEntry::PageRef(s) => s.to_string(),
                                    PropertyValueEntry::List(items) => items.join(", "),
                                };
                                (e.key.to_string(), value)
                            })
                            .collect();

                        CoreOperationResult::DocumentExport {
                            uri,
                            document_kind: Some(DocumentKind::Markdown),
                            headings,
                            xml_tags,
                            wiki_links,
                            markdown_links,
                            frontmatter,
                            properties,
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
                            frontmatter: Vec::new(),
                            properties: Vec::new(),
                        }
                    }
                    None => CoreOperationResult::Error(CoreError::Message(format!(
                        "document is not indexed: {}",
                        uri.as_str()
                    ))),
                }
            }
            CoreOperation::SearchWorkspace {
                query,
                frontmatter_filter,
                property_filter,
                tag_filter,
                realm: realm_name,
                limit,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(markymark_core::CoreError::Message(
                        format!("realm does not exist: {realm_key}"),
                    ));
                };
                crate::search::execute_search_workspace(
                    realm_key,
                    &realm_data.index,
                    query,
                    frontmatter_filter,
                    property_filter,
                    tag_filter,
                    limit,
                )
            }
            CoreOperation::SearchForPattern {
                pattern,
                include_glob,
                context_lines,
                limit,
                case_insensitive,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(markymark_core::CoreError::Message(
                        format!("realm does not exist: {realm_key}"),
                    ));
                };
                crate::pattern::execute_search_for_pattern(
                    realm_key,
                    &realm_data.index,
                    &pattern,
                    include_glob.as_deref(),
                    context_lines,
                    limit,
                    case_insensitive,
                )
            }
            CoreOperation::GraphAnalysis {
                realm: realm_name,
                top_n_hubs,
                include_clusters,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().expect("lock poisoned");
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(markymark_core::CoreError::Message(
                        format!("realm does not exist: {realm_key}"),
                    ));
                };
                crate::graph::execute_graph_analysis(
                    realm_key,
                    &realm_data.index,
                    top_n_hubs,
                    include_clusters,
                )
            }
        }
    }
}

fn total_tokens_for_realm(realm: &RealmIndex) -> (u64, usize) {
    let mut total_tokens = 0_u64;
    let mut unreadable_docs = 0_usize;
    for (uri, _) in realm.iter_all_documents() {
        let Some(path) = uri.to_file_path() else {
            unreadable_docs += 1;
            continue;
        };
        match fs::read_to_string(path) {
            Ok(source) => {
                total_tokens += u64::from(tokens::estimate_tokens(&source));
            }
            Err(_) => {
                unreadable_docs += 1;
            }
        }
    }
    (total_tokens, unreadable_docs)
}

#[cfg(feature = "semantic-search")]
fn preview_for_range(uri: &DocumentUri, range: Range, fallback: &str) -> String {
    let Some(path) = uri.to_file_path() else {
        return truncate_preview(fallback);
    };
    let Ok(source) = fs::read_to_string(path) else {
        return truncate_preview(fallback);
    };
    let Some(start_idx) = byte_offset_for_line(&source, range.start.line) else {
        return truncate_preview(fallback);
    };
    truncate_preview(&source[start_idx..])
}

#[cfg(feature = "semantic-search")]
fn byte_offset_for_line(source: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }

    let mut current_line = 0_u32;
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == line {
                return Some(idx + 1);
            }
        }
    }
    None
}

#[cfg(feature = "semantic-search")]
fn truncate_preview(text: &str) -> String {
    const MAX_PREVIEW_BYTES: usize = 200;
    let mut end = text.len().min(MAX_PREVIEW_BYTES);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].split_whitespace().collect::<Vec<_>>().join(" ")
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
