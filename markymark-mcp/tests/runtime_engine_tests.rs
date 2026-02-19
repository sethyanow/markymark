//! Integration tests for `RuntimeEngine` (the MCP core engine implementation).
//!
//! Extracted from `src/runtime_engine.rs` during the z6r refactor.
//! Split into submodules (marky-a90): each file covers one tool group.

use std::cmp::Ordering;
use std::path::PathBuf;

use markymark_core::Range;

#[path = "runtime_engine_tests/export_index.rs"]
mod export_index;
#[path = "runtime_engine_tests/find_references.rs"]
mod find_references;
#[path = "runtime_engine_tests/realm_isolation.rs"]
mod realm_isolation;
#[path = "runtime_engine_tests/realm_management.rs"]
mod realm_management;
#[path = "runtime_engine_tests/realm_stats.rs"]
mod realm_stats;
#[path = "runtime_engine_tests/rename.rs"]
mod rename;
#[path = "runtime_engine_tests/search_symbols.rs"]
mod search_symbols;
#[path = "runtime_engine_tests/search_workspace.rs"]
mod search_workspace;
#[path = "runtime_engine_tests/startup.rs"]
mod startup;

/// Compare two ranges for deterministic sorting (test-local copy).
pub(crate) fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
}

pub(crate) struct TempWorkspace {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl TempWorkspace {
    pub(crate) fn new(name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("markymark-mcp-runtime-{name}-"))
            .tempdir()
            .expect("secure temporary workspace directory should be created");
        let root = dir.path().to_path_buf();
        Self { _dir: dir, root }
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.root.clone()
    }
}
