//! Core engine: transport-agnostic operation types and trait.

use crate::{CoreError, DocumentUri, Range};

/// An operation that can be executed by the core engine.
#[derive(Debug)]
pub enum CoreOperation {
    /// Get the document outline (heading tree).
    GetOutline {
        /// Target document.
        uri: DocumentUri,
    },
    /// Find all references to the symbol at a position.
    FindReferences {
        /// Target document.
        uri: DocumentUri,
        /// Position of the symbol.
        position: Range,
    },
    /// Rename the symbol at a position.
    Rename {
        /// Target document.
        uri: DocumentUri,
        /// Position of the symbol.
        position: Range,
        /// New name for the symbol.
        new_name: String,
    },
    /// Search for symbols matching a query.
    SearchSymbols {
        /// Search query string.
        query: String,
    },
    /// Create a new named realm.
    CreateRealm {
        /// Realm name (must be unique).
        name: String,
    },
    /// Destroy a named realm and all its indexed documents.
    DestroyRealm {
        /// Realm name.
        name: String,
    },
    /// Add a workspace root to a realm and index its markdown files.
    AddRoot {
        /// Realm name.
        realm: String,
        /// Filesystem path to add.
        root: std::path::PathBuf,
    },
    /// Remove a workspace root from a realm, unindexing its documents.
    RemoveRoot {
        /// Realm name.
        realm: String,
        /// Filesystem path to remove.
        root: std::path::PathBuf,
    },
}

/// Transport-agnostic interface for executing core operations.
///
/// Both LSP and MCP transports call into this trait so indexing and
/// resolution logic stays shared in one place.
pub trait CoreEngine: Send + Sync {
    /// Execute a core operation and return the transport-neutral result.
    fn execute(&self, operation: CoreOperation) -> CoreOperationResult;
}

/// The result of a core engine operation.
#[derive(Debug)]
pub enum CoreOperationResult {
    /// An outline (list of heading descriptions).
    Outline(Vec<String>),
    /// A list of locations (uri, range).
    Locations(Vec<(DocumentUri, Range)>),
    /// A workspace edit (uri, list of (range, replacement text)).
    WorkspaceEdit(Vec<(DocumentUri, Vec<(Range, String)>)>),
    /// A list of symbols (name, uri, range).
    Symbols(Vec<(String, DocumentUri, Range)>),
    /// Realm info: name, root count, document count.
    RealmInfo {
        /// Realm name.
        name: String,
        /// Number of tracked workspace roots.
        root_count: usize,
        /// Number of indexed documents.
        document_count: usize,
    },
    /// Success with no payload.
    Ok,
    /// An error occurred.
    Error(CoreError),
}
