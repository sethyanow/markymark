//! Traits for generic connection graph node and edge types.
//!
//! Consumers implement [`EdgeKind`] and [`GraphNode`] for their domain-specific
//! types. markymark's own `RefKind` and `SymbolData` are the defaults.

use std::fmt::Debug;
use std::hash::Hash;

/// Trait for edge types in a `ConnectionGraph`.
///
/// Consumers implement this for their domain-specific edge semantics.
/// markymark's `RefKind` (document links) is the default implementation.
pub trait EdgeKind: Clone + Eq + Hash + Debug + Send + Sync {
    /// Whether this edge represents a blocking relationship.
    ///
    /// Used by graph utility methods like `blocking_predecessors()`.
    /// Document links (`RefKind`) are never blocking.
    /// Task dependencies (e.g. `DepType::Blocks`) can be blocking.
    fn is_blocking(&self) -> bool;
}

/// Trait for node types in a `ConnectionGraph`.
///
/// Provides a grouping key so that related nodes can be removed together.
/// For documents, the key is the document URI (removing a document removes
/// its headings too). For tasks, the key might be the task ID.
pub trait GraphNode: Clone + Debug + Send + Sync {
    /// The key type used for grouping and lookup.
    type Key: Hash + Eq + Clone + Debug;

    /// Return this node's grouping key.
    fn key(&self) -> Self::Key;
}
