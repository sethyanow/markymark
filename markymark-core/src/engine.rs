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
    /// Success with no payload.
    Ok,
    /// An error occurred.
    Error(CoreError),
}
