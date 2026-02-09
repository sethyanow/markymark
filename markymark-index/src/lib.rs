//! markymark-index: Document indexing and symbol resolution
//!
//! Provides document indexing, symbol lookup, and reference tracking.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod document;
pub mod graph;

pub use document::*;
pub use graph::*;
