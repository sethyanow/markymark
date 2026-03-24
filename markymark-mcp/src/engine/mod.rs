use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{RwLock, RwLockReadGuard};

use anyhow::bail;
use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::inference::InferenceProvider;
#[cfg(feature = "semantic-search")]
use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, RealmIndex, StructuredDocumentIndex};
use markymark_kernels::engine::DocumentEngine;
use markymark_parser::structured::parse_structured;

mod content_blocks;
mod curation;
mod diagnostics;
mod enrich;
mod export;
mod export_docs_index;
pub(crate) mod helpers;
mod outline;
mod realm_ops;
mod recommend;
mod references;
mod search;
mod search_block_text;
mod semantic_search;

/// The name of the default realm created at startup.
pub(crate) const DEFAULT_REALM: &str = "default";

/// Per-realm state: index, tracked workspace roots, and persistent document engines.
pub(crate) struct RealmData {
    pub(crate) index: RealmIndex,
    pub(crate) roots: Vec<PathBuf>,
    /// Persistent Zig document engines keyed by URI string.
    /// `DocumentEngine` is `Send` but NOT `Sync` (raw `*mut c_void` handle).
    /// Wrapping in `std::sync::Mutex` satisfies the `Sync` bound required by
    /// `RwLock<HashMap<String, RealmData>>` on `RuntimeEngine`.
    pub(crate) engines: HashMap<String, std::sync::Mutex<DocumentEngine>>,
}

impl RealmData {
    #[cfg(feature = "semantic-search")]
    pub(crate) fn new(provider: Option<Arc<dyn EmbeddingProvider>>) -> Self {
        Self {
            index: build_realm_index(provider),
            roots: Vec::new(),
            engines: HashMap::new(),
        }
    }

    #[cfg(not(feature = "semantic-search"))]
    pub(crate) fn new() -> Self {
        Self {
            index: build_realm_index(),
            roots: Vec::new(),
            engines: HashMap::new(),
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
    /// Inference provider for LLM-powered enrichment (optional).
    pub(crate) inference_provider: Option<Arc<dyn InferenceProvider>>,
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
            inference_provider: None,
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
            inference_provider: None,
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
            inference_provider: None,
        })
    }

    /// Resolve a realm name and acquire a mapped read lock on its data.
    ///
    /// Returns the resolved realm key and a read guard mapped directly to the
    /// `RealmData`. The guard holds the read lock for its lifetime.
    async fn read_realm(
        &self,
        realm_name: Option<&str>,
    ) -> Result<(String, RwLockReadGuard<'_, RealmData>), CoreOperationResult> {
        let realm_key = realm_name.unwrap_or(DEFAULT_REALM);
        let state = self.state.read().await;
        RwLockReadGuard::try_map(state, |s| s.get(realm_key))
            .map(|guard| (realm_key.to_string(), guard))
            .map_err(|_| {
                CoreOperationResult::Error(CoreError::Message(format!(
                    "realm does not exist: {realm_key}"
                )))
            })
    }
}

// -- Test hooks for forced failures (debug_assertions only) --

fn should_force_engine_create_fail_for_tests(uri_str: &str) -> bool {
    cfg!(debug_assertions) && uri_str.contains("__marky_test_force_create_fail__")
}

fn should_force_engine_update_fail_for_tests(uri_str: &str) -> bool {
    cfg!(debug_assertions) && uri_str.contains("__marky_test_force_update_fail__")
}

fn should_force_engine_result_conversion_fail_for_tests(uri_str: &str) -> bool {
    cfg!(debug_assertions) && uri_str.contains("__marky_test_force_conversion_fail__")
}

/// Build a [`DocumentIndex`] from raw text via an ephemeral engine.
///
/// This is the last-resort fallback when the persistent engine fails and
/// no stale index is cached. Panics on failure — acceptable because it's
/// the end of the fallback chain.
fn fallback_scan_with_frontmatter(text: &str) -> DocumentIndex {
    DocumentIndex::from_text(text)
}

/// Build a [`DocumentIndex`] from a persistent engine for a single markdown file.
///
/// Creates or updates the engine for `uri_str` in `realm.engines`, then converts
/// the engine result to a `DocumentIndex`. On failure, falls back to stale engine
/// snapshot first, then scan path if no stale state exists.
fn build_markdown_index_via_engine(
    realm: &mut RealmData,
    uri: &DocumentUri,
    source: &str,
) -> Option<DocumentIndex> {
    let uri_str = uri.as_str();
    let has_stale_index = realm.index.get_document(uri).is_some();

    let (fm, aliases) = markymark_index::parse_frontmatter_owned(source);
    let masked = markymark_index::mask_frontmatter(source);

    if let Some(engine_mutex) = realm.engines.get(uri_str) {
        // Engine exists — update it.
        let mut engine = match engine_mutex.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => {
                log::warn!(
                    target: "markymark_mcp",
                    "engine mutex poisoned for {}",
                    uri_str
                );
                return if has_stale_index {
                    None
                } else {
                    Some(fallback_scan_with_frontmatter(source))
                };
            }
        };
        let update_result = if should_force_engine_update_fail_for_tests(uri_str) {
            Err("forced engine update failure (test hook)".to_string())
        } else {
            engine
                .update(&masked, None)
                .map_err(|e| format!("engine update failed: {e:?}"))
        };

        match update_result {
            Ok(()) => {
                match index_from_engine_result(uri_str, &engine, fm.clone(), aliases.clone(), source.to_string()) {
                    Ok(index) => return Some(index),
                    Err(e) => {
                        log::warn!(
                            target: "markymark_mcp",
                            "engine result conversion failed for {}: {}, trying stale fallback",
                            uri_str, e
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    target: "markymark_mcp",
                    "{} for {}, trying stale engine snapshot",
                    e, uri_str
                );
            }
        }

        // Stale engine snapshot fallback — get_result() returns last successful parse.
        match index_from_engine_result(uri_str, &engine, fm.clone(), aliases.clone(), source.to_string()) {
            Ok(index) => return Some(index),
            Err(e) => {
                log::warn!(
                    target: "markymark_mcp",
                    "stale engine snapshot failed for {}: {}",
                    uri_str, e
                );
            }
        }
    } else {
        // No engine yet — create one with masked text.
        if should_force_engine_create_fail_for_tests(uri_str) {
            log::warn!(
                target: "markymark_mcp",
                "forced engine create failure (test hook) for {}",
                uri_str
            );
        } else {
            match DocumentEngine::new(&masked) {
                Ok(engine) => {
                    let built = index_from_engine_result(uri_str, &engine, fm, aliases, source.to_string());
                    realm
                        .engines
                        .insert(uri_str.to_string(), std::sync::Mutex::new(engine));
                    match built {
                        Ok(index) => return Some(index),
                        Err(e) => {
                            log::warn!(
                                target: "markymark_mcp",
                                "engine result conversion failed (new engine) for {}: {}, using fallback",
                                uri_str, e
                            );
                        }
                    }
                }
                Err(e) => log::warn!(
                    target: "markymark_mcp",
                    "engine create failed for {}: {:?}",
                    uri_str, e
                ),
            }
        }
    }

    // Final fallback: stale index (None preserves existing) or scan path.
    if has_stale_index {
        None
    } else {
        Some(fallback_scan_with_frontmatter(source))
    }
}

/// Convert engine result to a DocumentIndex with frontmatter and source text.
///
/// When `source` is provided, the resulting index retains source text for
/// `block_text()` and extracts content blocks via tree-sitter block parsing.
fn index_from_engine_result(
    uri_str: &str,
    engine: &DocumentEngine,
    frontmatter: Vec<markymark_index::FrontmatterOwnedEntry>,
    aliases: Vec<String>,
    source: String,
) -> Result<DocumentIndex, String> {
    let result = engine
        .get_result()
        .map_err(|e| format!("get_result failed: {e:?}"))?;
    if should_force_engine_result_conversion_fail_for_tests(uri_str) {
        return Err("forced engine result conversion failure (test hook)".to_string());
    }
    let extraction = result
        .to_extraction()
        .map_err(|e| format!("to_extraction failed: {e:?}"))?;
    Ok(DocumentIndex::from_engine_result_with_source(
        &extraction,
        frontmatter,
        aliases,
        source,
    ))
}

/// Index all markdown files under a root into a realm.
///
/// Markdown documents use persistent Zig DocumentEngines for extraction via
/// CEngineResult. Each file gets a persistent engine in `realm.engines`.
/// On engine failure, falls back to stale engine snapshot, then scan path.
/// Structured documents (JSON, YAML, TOML, etc.) still use tree-sitter via
/// `StructuredDocumentIndex::from_ast`.
pub(crate) async fn index_root_into_realm(root: &Path, realm: &mut RealmData) {
    let documents = helpers::collect_documents(root);
    let mut markdown_docs = Vec::new();

    for (path, kind) in documents {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let uri = DocumentUri::from_file_path(&path);

        if kind == DocumentKind::Markdown {
            if let Some(index) = build_markdown_index_via_engine(realm, &uri, &source) {
                markdown_docs.push((uri, index));
            }
            // None means stale index preserved — skip re-adding.
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

    if !markdown_docs.is_empty() {
        realm.index.add_documents(markdown_docs).await;
    }
}

/// Remove all documents under a root from a realm's index and clean up engines.
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

    for uri in &to_remove {
        realm.index.remove_document(uri).await;
        // Clean up persistent engine — DocumentEngine::drop calls marky_engine_destroy.
        realm.engines.remove(uri.as_str());
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
                format,
                include_text,
            } => {
                let (_realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                outline::handle_get_outline(
                    &realm_data.index,
                    &realm_data.roots,
                    &uri,
                    &format,
                    include_text,
                )
            }
            CoreOperation::SearchSymbols {
                query,
                realm: realm_name,
            } => {
                let (_realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                search::handle_search_symbols(&realm_data.index, query)
            }
            CoreOperation::SemanticSearch {
                query,
                realm,
                top_k,
                min_score,
            } => {
                self.handle_semantic_search(query, realm, top_k, min_score)
                    .await
            }
            CoreOperation::FindReferences {
                uri,
                position,
                realm: realm_name,
            } => {
                let (_realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                references::handle_find_references(&realm_data.index, &uri, position)
            }
            CoreOperation::Rename {
                uri,
                position,
                new_name,
                realm: realm_name,
            } => {
                let (_realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
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

                // Index all documents under the root using persistent engines.
                // This holds the write lock during file I/O — acceptable because
                // AddRoot is rare (user-initiated) and startup uses the same pattern.
                let mut state = self.state.write().await;
                let realm_data = match state.get_mut(&realm) {
                    Some(data) => data,
                    None => {
                        return CoreOperationResult::Error(CoreError::Message(format!(
                            "realm not found after registration: {realm}"
                        )));
                    }
                };

                index_root_into_realm(&root, realm_data).await;

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
                let (realm_key, realm_data) = match self.read_realm(Some(realm.as_str())).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                realm_ops::handle_realm_stats(
                    &realm_data,
                    realm_key,
                    check_duplicates,
                    include_token_counts,
                )
                .await
            }
            CoreOperation::DependencyGraph { realm, format } => {
                let (_realm_key, realm_data) = match self.read_realm(Some(realm.as_str())).await {
                    Ok(v) => v,
                    Err(e) => return e,
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
                include_blocks,
            } => {
                let (_realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                export::handle_export_index(&realm_data.index, &uri, include_blocks)
            }
            CoreOperation::SearchWorkspace {
                query,
                frontmatter_filter,
                property_filter,
                tag_filter,
                realm: realm_name,
                limit,
            } => {
                let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                crate::search::execute_search_workspace(
                    &realm_key,
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
                let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                crate::pattern::execute_search_for_pattern(
                    &realm_key,
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
                let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                crate::graph::execute_graph_analysis(
                    &realm_key,
                    &realm_data.index,
                    top_n_hubs,
                    include_clusters,
                )
            }
            CoreOperation::GetDiagnostics {
                uri,
                realm: realm_name,
            } => {
                let (realm_key, realm_data) = match self.read_realm(realm_name.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                match uri {
                    Some(uri) => {
                        diagnostics::handle_get_diagnostics_file(&realm_data, &realm_key, &uri)
                    }
                    None => diagnostics::handle_get_diagnostics_realm(&realm_data, &realm_key),
                }
            }
            CoreOperation::ExportDocsIndex {
                realm,
                name_override,
            } => {
                let realm_key = realm.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                export_docs_index::handle_export_docs_index(
                    realm_data,
                    realm_key.to_string(),
                    name_override,
                )
            }
            CoreOperation::EnrichDocument {
                uri,
                realm,
                sidecar_dir,
                force,
            } => {
                let realm_key = realm.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                enrich::handle_enrich_document(
                    &realm_data.index,
                    &realm_data.roots,
                    &uri,
                    sidecar_dir.as_deref(),
                    force,
                    self.inference_provider.as_deref(),
                )
                .await
            }
            CoreOperation::RecommendDocs {
                query,
                realm,
                top_k,
                include_sections,
            } => {
                let realm_key = realm.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                recommend::handle_recommend_docs(
                    realm_key,
                    &realm_data.index,
                    &realm_data.roots,
                    &query,
                    top_k,
                    include_sections,
                )
            }
            CoreOperation::CurationDiagnostics {
                realm,
                include_suggestions,
                max_suggestions,
                max_items_per_category,
            } => {
                let realm_key = realm.as_deref().unwrap_or(DEFAULT_REALM);
                let state = self.state.read().await;
                let Some(realm_data) = state.get(realm_key) else {
                    return CoreOperationResult::Error(CoreError::Message(format!(
                        "realm does not exist: {realm_key}"
                    )));
                };
                curation::handle_curation_diagnostics(
                    realm_key,
                    &realm_data.index,
                    include_suggestions,
                    max_suggestions,
                    max_items_per_category,
                )
            }
            CoreOperation::GetContentBlocks {
                uri,
                realm: realm_name,
                kind_filter,
                heading_filter,
                block_id,
                include_text,
            } => {
                self.handle_get_content_blocks(
                    uri,
                    realm_name,
                    kind_filter,
                    heading_filter,
                    block_id,
                    include_text,
                )
                .await
            }

            CoreOperation::SearchBlockText {
                query,
                realm: realm_name,
                kind_filter,
                limit,
                include_text,
            } => {
                self.handle_search_block_text(query, realm_name, kind_filter, limit, include_text)
                    .await
            }
        }
    }
}

// Standalone helpers moved to engine/helpers.rs

#[cfg(test)]
mod tests;
