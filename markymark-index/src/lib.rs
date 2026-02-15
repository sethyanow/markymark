//! markymark-index: Document indexing and symbol resolution
//!
//! Provides document indexing, symbol lookup, and reference tracking.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod bench_config;
pub mod bench_corpus;
pub mod bench_report;
pub mod document;
pub mod graph;
pub mod realm;
pub mod resolution;

pub use bench_config::*;
pub use bench_corpus::*;
pub use bench_report::*;
pub use document::*;
pub use graph::*;
pub use realm::*;
pub use resolution::*;
