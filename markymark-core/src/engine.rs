//! Core engine: transport-agnostic operation types and trait.

use crate::structured::DocumentKind;
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
    /// Get aggregate statistics for a named realm.
    RealmStats {
        /// Realm name (e.g. "default").
        realm: String,
    },
    /// Export the full document index for a single document.
    ExportIndex {
        /// Target document.
        uri: DocumentUri,
    },
    /// Get a dependency graph showing inter-document links.
    DependencyGraph {
        /// Realm name (e.g. "default").
        realm: String,
        /// Output format: "json" or "dot".
        format: String,
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
    /// Realm statistics: aggregate counts across all indexed documents.
    RealmStats {
        /// Realm name.
        name: String,
        /// Number of tracked workspace roots.
        root_count: usize,
        /// Number of indexed documents.
        document_count: usize,
        /// Total headings across all documents.
        heading_count: usize,
        /// Total XML tags across all documents.
        xml_tag_count: usize,
        /// Total wiki links across all documents.
        wiki_link_count: usize,
        /// Total markdown links across all documents.
        markdown_link_count: usize,
        /// Number of structured (non-markdown) documents indexed.
        structured_doc_count: usize,
        /// Total key paths across all structured documents.
        key_path_count: usize,
    },
    /// Exported document index: full structured data for a single document.
    DocumentExport {
        /// Document URI.
        uri: DocumentUri,
        /// The kind of document (Markdown, JSON, YAML, etc.).
        document_kind: Option<DocumentKind>,
        /// Heading texts and levels.
        headings: Vec<(String, u8, Range)>,
        /// XML tag names with ranges.
        xml_tags: Vec<(String, Range)>,
        /// Wiki link targets with ranges.
        wiki_links: Vec<(String, Option<String>, Range)>,
        /// Markdown link URLs with ranges.
        markdown_links: Vec<(String, String, Range)>,
    },
    /// A dependency graph in the requested format (json or dot).
    DependencyGraph {
        /// Realm name.
        realm: String,
        /// Output format used.
        format: String,
        /// Serialized graph content.
        content: String,
    },
    /// Success with no payload.
    Ok,
    /// An error occurred.
    Error(CoreError),
}
