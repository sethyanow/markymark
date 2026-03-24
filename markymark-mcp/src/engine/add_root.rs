use std::fs;
use std::path::PathBuf;

use markymark_core::engine::CoreOperationResult;
use markymark_core::scanner::Md4cScanBackend;
use markymark_core::structured::DocumentKind;
use markymark_core::{CoreError, DocumentUri};
use markymark_index::{DocumentIndex, StructuredDocumentIndex};
use markymark_parser::structured::parse_structured;

use super::{helpers, realm_ops, RuntimeEngine};

impl RuntimeEngine {
    /// Handle the `AddRoot` operation: validate, parse, embed, and index all
    /// documents under the given root path within the specified realm.
    ///
    /// Uses a 4-phase locking protocol to minimize lock contention:
    /// 1. Write lock — validate and register root (fast, sync)
    /// 2. No lock — collect and parse documents (slow, I/O-bound)
    /// 3. No outer lock — semantic embedding (slow, network I/O) [cfg-gated]
    /// 4. Write lock — structural index update (fast, in-memory)
    pub(super) async fn handle_add_root(
        &self,
        realm: String,
        root: PathBuf,
    ) -> CoreOperationResult {
        // Phase 1: validate and register root (write lock, fast sync).
        {
            let mut state = self.state.write().await;
            if let Err(e) = realm_ops::validate_and_register_root(&mut state, &realm, &root) {
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
                let (fm_owned, aliases_owned) = markymark_index::parse_frontmatter_owned(&source);
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
        // Uses batch API to reduce mutex contention and enable batched
        // embedding calls (single HTTP request for Voyage, batched ONNX
        // inference for local).
        #[cfg(feature = "semantic-search")]
        let semantic_arc = {
            let state = self.state.read().await;
            state
                .get(&realm)
                .and_then(|rd| rd.index.semantic_index_arc())
        };
        #[cfg(feature = "semantic-search")]
        if let Some(ref sem) = semantic_arc {
            let docs_refs: Vec<_> = parsed_md
                .iter()
                .map(|(uri, doc)| (uri.clone(), doc as &DocumentIndex))
                .collect();
            let mut guard = sem.lock().await;
            if let Err(err) = guard.add_documents(docs_refs).await {
                eprintln!("warning: batch semantic indexing failed: {err}",);
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

        // Root may have been removed while Phase 2/3 ran without the write lock.
        let root_canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        let root_still_present = realm_data.roots.iter().any(|existing| {
            existing.canonicalize().unwrap_or_else(|_| existing.clone()) == root_canonical
        });
        if !root_still_present {
            // Clean up semantic entries that Phase 3 may have added
            // for this root's documents. Without this, stale semantic
            // entries remain searchable after root removal.
            #[cfg(feature = "semantic-search")]
            if let Some(ref sem) = semantic_arc {
                let mut guard = sem.lock().await;
                for (uri, _) in &parsed_md {
                    guard.remove_document(uri);
                }
            }

            log::warn!(
                "root removed during indexing; realm={realm}, root={}, discarding {} parsed documents",
                root.display(),
                parsed_md.len() + parsed_struct.len()
            );
            return CoreOperationResult::RealmInfo {
                name: realm.clone(),
                root_count: realm_data.roots.len(),
                document_count: realm_data.index.document_count(),
            };
        }

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
}
