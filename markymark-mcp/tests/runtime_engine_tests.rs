//! Integration tests for `RuntimeEngine` (the MCP core engine implementation).
//!
//! Extracted from `src/runtime_engine.rs` during the z6r refactor.
//! Split into submodules (marky-a90): each file covers one tool group.

mod common;

use std::cmp::Ordering;

use markymark_core::Range;

pub(crate) use common::TempWorkspace;

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
#[path = "runtime_engine_tests/content_blocks.rs"]
mod content_blocks;

/// Compare two ranges for deterministic sorting (test-local copy).
pub(crate) fn compare_ranges(a: Range, b: Range) -> Ordering {
    a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end))
}
