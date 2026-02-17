//! markymark-index: Document indexing and symbol resolution
//!
//! Provides document indexing, symbol lookup, and reference tracking.

#![warn(missing_docs)]
#![warn(clippy::all)]

#[cfg(feature = "bench-internals")]
pub mod bench_config;
#[cfg(feature = "bench-internals")]
pub mod bench_corpus;
#[cfg(feature = "bench-internals")]
pub mod bench_report;
pub mod document;
pub mod graph;
pub mod realm;
pub mod resolution;
#[cfg(feature = "embeddings")]
pub mod semantic;
pub mod structured_document;

#[cfg(feature = "bench-internals")]
pub use bench_config::*;
#[cfg(feature = "bench-internals")]
pub use bench_corpus::*;
#[cfg(feature = "bench-internals")]
pub use bench_report::*;
pub use document::*;
pub use graph::*;
pub use realm::*;
pub use resolution::*;
#[cfg(feature = "embeddings")]
pub use semantic::*;
pub use structured_document::*;
