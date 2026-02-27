use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(feature = "semantic-search")]
use std::sync::Arc;

use tokio::sync::RwLock;

use anyhow::bail;
use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
#[cfg(feature = "semantic-search")]
use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use markymark_core::scanner::Md4cScanBackend;
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, RealmIndex, StructuredDocumentIndex};
use markymark_parser::structured::parse_structured;

mod diagnostics;
mod export;
mod helpers;
mod outline;
mod realm_ops;
mod references;
mod search;

/// The name of the default realm created at startup.
pub(crate) const DEFAULT_REALM: &str = "default";

/// Per-realm state: index plus tracked workspace roots.
pub(crate) struct RealmData {
    pub(crate) index: RealmIndex,
    pub(crate) roots: Vec<PathBuf>,
}

impl RealmData {
    #[cfg(feature = "semantic-search")]
    pub(crate) fn new(provider: Option<Arc<dyn EmbeddingProvider>>) -> Self {
        Self {
            index: build_realm_index(provider),
            roots: Vec::new(),
        }
    }

    #[cfg(not(feature = "semantic-search"))]
    pub(crate) fn new() -> Self {
        Self {
            index: build_realm_index(),
            roots: Vec::new(),
        }
    }
}

#[cfg(feature = "semantic-search")]
pub(crate) fn build_realm_index(provider: Option<Arc<dyn EmbeddingProvider>>) -> RealmIndex {
    match provider {
        Some(p) => RealmIndex::new_with_embeddings(p).unwrap_or_else(|err| {
            eprintln!("warning: failed to initialize semantic index; falling back to plain realm index: {err}");
            RealmIndex::new()
        }),
        None => RealmIndex::new(),
    }
}

#[cfg(not(feature = "semantic-search"))]
pub(crate) fn build_realm_index() -> RealmIndex {
    RealmIndex::new()
}

/// FNV-1a 32-bit hash for stable, cross-version token hashing.
///
/// `DefaultHasher` (SipHash 1-3) is explicitly not guaranteed to be stable
/// across Rust versions. FNV-1a is a well-specified algorithm that produces
/// identical output forever for the same input, making it appropriate for
/// the bag-of-words hash embedding used in [`HashEmbeddingProvider`].
#[cfg(feature = "semantic-search")]
fn fnv1a32(bytes: &[u8]) -> u32 {
    const OFFSET: u32 = 0x811c9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut hash = OFFSET;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(feature = "semantic-search")]
#[derive(Debug, Clone)]
/// Dev/test hash-based embedding provider using FNV-1a bag-of-words.
///
/// Not suitable for production semantic search — use a real provider like
/// [`VoyageProvider`](markymark_core::embeddings::voyage::VoyageProvider) instead.
pub struct HashEmbeddingProvider {
    dims: u32,
}

#[cfg(feature = "semantic-search")]
impl HashEmbeddingProvider {
    /// Create a new hash embedding provider with the given dimensionality.
    pub fn new(dims: u32) -> Self {
        Self { dims }
    }
}

#[cfg(feature = "semantic-search")]
#[async_trait]
impl EmbeddingProvider for HashEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
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
            let idx = (fnv1a32(norm.as_bytes()) as usize) % out.len();
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
    pub(crate) state: RwLock<HashMap<String, RealmData>>,
    /// Embedding provider shared across all realms (only when semantic-search feature enabled).
    #[cfg(feature = "semantic-search")]
    pub(crate) provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl Default for RuntimeEngine {
    fn default() -> Self {
        let mut realms = HashMap::new();
        #[cfg(feature = "semantic-search")]
        realms.insert(DEFAULT_REALM.to_string(), RealmData::new(None));
        #[cfg(not(feature = "semantic-search"))]
        realms.insert(DEFAULT_REALM.to_string(), RealmData::new());
        Self {
            state: RwLock::new(realms),
            #[cfg(feature = "semantic-search")]
            provider: None,
        }
    }
}

impl RuntimeEngine {
    /// Build a runtime engine from workspace roots.
    ///
    /// All markdown files (`*.md`, `*.markdown`) under the provided roots are indexed
    /// into the "default" realm.
    /// Invalid roots fail startup. Individual document read/parse failures are skipped.
    pub async fn from_workspace_roots(workspace_roots: Vec<PathBuf>) -> anyhow::Result<Self> {
        if workspace_roots.is_empty() {
            bail!("at least one workspace root is required");
        }

        #[cfg(feature = "semantic-search")]
        let mut default_realm = RealmData::new(None);
        #[cfg(not(feature = "semantic-search"))]
        let mut default_realm = RealmData::new();

        for root in workspace_roots {
            helpers::validate_workspace_root(&root)?;
            index_root_into_realm(&root, &mut default_realm).await;
            default_realm.roots.push(root);
        }

        let mut realms = HashMap::new();
        realms.insert(DEFAULT_REALM.to_string(), default_realm);

        Ok(Self {
            state: RwLock::new(realms),
            #[cfg(feature = "semantic-search")]
            provider: None,
        })
    }

    /// Build a runtime engine from workspace roots with an explicit embedding provider.
    ///
    /// When `provider` is `Some`, all realms (including dynamically-created ones) will
    /// be initialized with semantic search support using that provider.
    #[cfg(feature = "semantic-search")]
    pub async fn from_workspace_roots_with_provider(
        workspace_roots: Vec<PathBuf>,
        provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> anyhow::Result<Self> {
        if workspace_roots.is_empty() {
            bail!("at least one workspace root is required");
        }

        let mut default_realm = RealmData::new(provider.clone());

        for root in workspace_roots {
            helpers::validate_workspace_root(&root)?;
            index_root_into_realm(&root, &mut default_realm).await;
            default_realm.roots.push(root);
        }

        let mut realms = HashMap::new();
        realms.insert(DEFAULT_REALM.to_string(), default_realm);

        Ok(Self {
            state: RwLock::new(realms),
            provider,
        })
    }
}

/// Index all markdown files under a root into a realm.
///
/// Markdown documents use the Zig scan path (`from_scan_with_frontmatter`) for
/// full extraction including code spans, tasks, embeds, callouts, etc.
/// Frontmatter is parsed directly from source text (no tree-sitter needed).
/// Structured documents (JSON, YAML, TOML, etc.) still use tree-sitter via
/// `StructuredDocumentIndex::from_ast`.
pub(crate) async fn index_root_into_realm(root: &Path, realm: &mut RealmData) {
    let backend = Md4cScanBackend;
    let documents = helpers::collect_documents(root);
    let mut parsed_markdown: Vec<(DocumentUri, DocumentIndex)> = Vec::new();
    let mut parsed_structured: Vec<(DocumentUri, StructuredDocumentIndex)> = Vec::new();

    for (path, kind) in documents {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let uri = DocumentUri::from_file_path(&path);

        if kind == DocumentKind::Markdown {
            let (fm_owned, aliases_owned) = markymark_index::parse_frontmatter_owned(&source);

            // Mask frontmatter block so md4c doesn't misparse `---` as a
            // setext heading underline. Replace non-newline bytes with spaces
            // to preserve line counting and byte offsets.
            let scan_source = markymark_index::mask_frontmatter(&source);
            parsed_markdown.push((
                uri,
                DocumentIndex::from_scan_with_frontmatter(
                    &scan_source,
                    &backend,
                    fm_owned,
                    aliases_owned,
                ),
            ));
        } else {
            let ast = match parse_structured(&source, kind) {
                Ok(ast) => ast,
                Err(_) => continue,
            };
            parsed_structured.push((uri, StructuredDocumentIndex::from_ast(ast)));
        }
    }

    realm.index.add_documents(parsed_markdown).await;
    for (uri, doc) in parsed_structured {
        realm.index.add_structured_document(uri, doc);
    }
}

/// Remove all documents under a root from a realm's index.
pub(crate) async fn unindex_root_from_realm(root: &Path, realm: &mut RealmData) {
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
        realm.index.remove_document(&uri).await;
    }
}

#[async_trait]
impl CoreEngine for RuntimeEngine {
    async fn execute(&self, operation: CoreOperation) -> CoreOperationResult {
        match operation {
            // --- Document operations (read from specified realm, falling back to default) ---
            CoreOperation::GetOutline {
                uri,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                outline::handle_get_outline(&realm_data.index, &uri)
            }
            CoreOperation::SearchSymbols {
                query,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                search::handle_search_symbols(&realm_data.index, query)
            }
            CoreOperation::SemanticSearch {
                query,
                realm,
                top_k,
                min_score,
            } => {
                #[cfg(not(feature = "semantic-search"))]
                {
                    let realm_name = realm.unwrap_or_else(|| DEFAULT_REALM.to_string());
                    let _ = (realm_name, query, top_k, min_score);
                    CoreOperationResult::Error(CoreError::NotImplemented(
                        "semantic-search feature is not enabled for markymark-mcp".to_string(),
                    ))
                }

                #[cfg(feature = "semantic-search")]
                {
                    let realm_name = realm.unwrap_or_else(|| DEFAULT_REALM.to_string());
                    // Phase 1: acquire read lock, clone the Arc handle, release read lock.
                    let semantic_arc = {
                        let state = self.state.read().await;
                        let realm_data = match state.get(&realm_name) {
                            Some(data) => data,
                            None => {
                                return CoreOperationResult::Error(CoreError::Message(format!(
                                    "realm does not exist: {realm_name}"
                                )));
                            }
                        };
                        match realm_data.index.semantic_index_arc() {
                            Some(arc) => arc,
                            None => {
                                return CoreOperationResult::Error(CoreError::Message(
                                    "semantic search is not configured for this realm".to_string(),
                                ));
                            }
                        }
                        // state (read guard) dropped here at end of block
                    };

                    // Phase 2: search with the Arc — no outer lock held.
                    search::handle_semantic_search(semantic_arc, query, top_k, min_score).await
                }
            }
            CoreOperation::FindReferences {
                uri,
                position,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                references::handle_find_references(&realm_data.index, &uri, position)
            }
            CoreOperation::Rename {
                uri,
                position,
                new_name,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                references::handle_rename(&realm_data.index, &uri, position, &new_name)
            }

            // --- Realm management operations ---
            CoreOperation::CreateRealm { name } => {
                let mut state = self.state.write().await;
                #[cfg(feature = "semantic-search")]
                {
                    realm_ops::handle_create_realm(&mut state, name, self.provider.clone())
                }
                #[cfg(not(feature = "semantic-search"))]
                {
                    realm_ops::handle_create_realm(&mut state, name)
                }
            }
            CoreOperation::DestroyRealm { name } => {
                let mut state = self.state.write().await;
                realm_ops::handle_destroy_realm(&mut state, name)
            }
            CoreOperation::AddRoot { realm, root } => {
                // Phase 1: validate and register root (write lock, fast sync).
                {
                    let mut state = self.state.write().await;
                    if let Err(e) = realm_ops::validate_and_register_root(&mut state, &realm, &root)
                    {
                        return CoreOperationResult::Error(e);
                    }
                } // write lock released

                // Phase 2: collect + parse documents (no lock, I/O-bound).
                let backend = Md4cScanBackend;
                let doc_paths = helpers::collect_documents(&root);
                let mut parsed_md: Vec<(DocumentUri, DocumentIndex)> = Vec::new();
                let mut parsed_struct: Vec<(DocumentUri, StructuredDocumentIndex)> = Vec::new();

                for (path, kind) in doc_paths {
                    let source = match fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let uri = DocumentUri::from_file_path(&path);

                    if kind == DocumentKind::Markdown {
                        let (fm_owned, aliases_owned) =
                            markymark_index::parse_frontmatter_owned(&source);
                        let scan_source = markymark_index::mask_frontmatter(&source);
                        parsed_md.push((
                            uri,
                            DocumentIndex::from_scan_with_frontmatter(
                                &scan_source,
                                &backend,
                                fm_owned,
                                aliases_owned,
                            ),
                        ));
                    } else {
                        let ast = match parse_structured(&source, kind) {
                            Ok(ast) => ast,
                            Err(_) => continue,
                        };
                        parsed_struct.push((uri, StructuredDocumentIndex::from_ast(ast)));
                    }
                }

                // Phase 3: semantic embedding (no outer lock, slow network I/O).
                #[cfg(feature = "semantic-search")]
                {
                    let semantic_arc = {
                        let state = self.state.read().await;
                        state
                            .get(&realm)
                            .and_then(|rd| rd.index.semantic_index_arc())
                    };
                    if let Some(sem) = semantic_arc {
                        let semantic_docs: Vec<(DocumentUri, &DocumentIndex)> = parsed_md
                            .iter()
                            .map(|(uri, doc)| (uri.clone(), doc))
                            .collect();
                        let mut guard = sem.lock().await;
                        if let Err(err) = guard.add_documents(semantic_docs).await {
                            eprintln!(
                                "warning: semantic indexing failed for realm {}: {err}",
                                realm
                            );
                        }
                    }
                }

                // Phase 4: structural index update (write lock, fast in-memory ops).
                let mut state = self.state.write().await;
                let realm_data = match state.get_mut(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm was destroyed during indexing: {realm}"
                        )));
                    }
                };
                for (uri, doc) in parsed_md {
                    realm_data.index.add_document_structural(uri, doc);
                }
                for (uri, doc) in parsed_struct {
                    realm_data.index.add_structured_document(uri, doc);
                }

                CoreOperationResult::RealmInfo {
                    name: realm.clone(),
                    root_count: realm_data.roots.len(),
                    document_count: realm_data.index.document_count(),
                }
            }
            CoreOperation::RemoveRoot { realm, root } => {
                let mut state = self.state.write().await;
                realm_ops::handle_remove_root(&mut state, realm, root).await
            }

            // --- Query operations ---
            CoreOperation::RealmStats {
                realm,
                check_duplicates,
                include_token_counts,
            } => {
                let state = self.state.read().await;
                let realm_data = match state.get(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };
                realm_ops::handle_realm_stats(
                    realm_data,
                    realm,
                    check_duplicates,
                    include_token_counts,
                )
                .await
            }
            CoreOperation::DependencyGraph { realm, format } => {
                let state = self.state.read().await;
                let realm_data = match state.get(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm does not exist: {realm}"
                        )));
                    }
                };

                let content = helpers::build_dependency_graph(&realm_data.index, &format);
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
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                export::handle_export_index(&realm_data.index, &uri)
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
                let state = self.state.read().await;
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
                let state = self.state.read().await;
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
                let state = self.state.read().await;
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
            CoreOperation::GetDiagnostics {
                uri,
                realm: realm_name,
            } => {
                let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(markymark_core::CoreError::Message(
                        format!("realm does not exist: {realm_key}"),
                    ));
                };
                match uri {
                    Some(uri) => {
                        diagnostics::handle_get_diagnostics_file(realm_data, realm_key, &uri)
                    }
                    None => diagnostics::handle_get_diagnostics_realm(realm_data, realm_key),
                }
            }
        }
    }
}

// Standalone helpers moved to engine/helpers.rs

#[cfg(test)]
mod tests;
