use super::*;
use markymark_core::{Position, Range};
use std::fs;

fn make_temp_realm_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

async fn make_engine_with_custom_realm(realm_name: &str, dir: &Path) -> RuntimeEngine {
    let engine = RuntimeEngine::default();
    // create the realm
    engine
        .execute(CoreOperation::CreateRealm {
            name: realm_name.to_string(),
        })
        .await;
    // index the directory into it
    engine
        .execute(CoreOperation::AddRoot {
            realm: realm_name.to_string(),
            root: dir.to_path_buf(),
        })
        .await;
    engine
}

#[cfg(feature = "semantic-search")]
mod concurrency;

mod curation;
mod engine_indexing;
mod enrich;
mod export_docs_index;
mod export_index;
mod find_references;
mod from_text_equivalence;
#[cfg(feature = "semantic-search")]
mod hash_embedding;
mod outline;
mod recommend;
mod rename;
mod search_symbols;
mod workspace_scan;

#[cfg(feature = "semantic-search")]
mod preview_profiling;
