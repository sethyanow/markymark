//! markymark CLI entry point.
//!
//! Supports both LSP and MCP transport modes via `--lsp` / `--mcp` flags.
//! Defaults to LSP when neither flag is specified.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use clap::Parser;

/// Runtime semantic search provider selection.
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum SemanticProvider {
    /// Voyage AI API (requires VOYAGE_API_KEY env var).
    Voyage,
    /// Local ONNX inference (all-MiniLM-L6-v2, no API key needed).
    Local,
    /// Dev/test hash-based embedding provider.
    Hash,
}

/// markymark — high-performance Markdown language tooling.
///
/// Runs as an LSP server (default) or MCP server for AI assistants.
/// Pass workspace roots as positional arguments, or defaults to the
/// current working directory.
#[derive(Parser, Debug)]
#[command(name = "markymark", version, about)]
struct Cli {
    /// Run in LSP (Language Server Protocol) mode [default].
    #[arg(long, conflicts_with = "mcp")]
    lsp: bool,

    /// Run in MCP (Model Context Protocol) mode for AI assistants.
    #[arg(long, conflicts_with = "lsp")]
    mcp: bool,

    /// Enable semantic search with the given provider.
    ///
    /// Only meaningful with `--mcp`. Ignored in LSP mode.
    /// Requires the `semantic-search` feature (and `voyage` for Voyage provider).
    #[arg(long, value_enum)]
    semantic_search: Option<SemanticProvider>,

    /// Workspace root directories to index.
    ///
    /// If omitted, defaults to the current working directory.
    #[arg(name = "ROOTS")]
    roots: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let roots = if cli.roots.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        cli.roots
    };

    if cli.mcp {
        run_mcp(roots, cli.semantic_search).await
    } else {
        // Default to LSP mode (when neither flag or --lsp is specified).
        if cli.semantic_search.is_some() {
            eprintln!("warning: --semantic-search is only supported in MCP mode; ignoring");
        }
        run_lsp().await
    }
}

async fn run_lsp() -> Result<()> {
    markymark_lsp::run_stdio().await;
    Ok(())
}

async fn run_mcp(roots: Vec<PathBuf>, semantic: Option<SemanticProvider>) -> Result<()> {
    let engine = match semantic {
        Some(provider) => build_engine_with_provider(roots, provider).await?,
        None => markymark_mcp::RuntimeEngine::from_workspace_roots(roots).await?,
    };
    markymark_mcp::run_stdio(Arc::new(engine)).await
}

#[cfg(feature = "semantic-search")]
async fn build_engine_with_provider(
    roots: Vec<PathBuf>,
    provider: SemanticProvider,
) -> Result<markymark_mcp::RuntimeEngine> {
    use markymark_core::prelude::EmbeddingProvider;

    let embedding: Arc<dyn EmbeddingProvider> = match provider {
        SemanticProvider::Voyage => build_voyage_provider()?,
        SemanticProvider::Local => build_local_provider()?,
        SemanticProvider::Hash => Arc::new(markymark_mcp::HashEmbeddingProvider::new(128)),
    };
    markymark_mcp::RuntimeEngine::from_workspace_roots_with_provider(roots, Some(embedding))
        .await
}

#[cfg(not(feature = "semantic-search"))]
async fn build_engine_with_provider(
    _roots: Vec<PathBuf>,
    provider: SemanticProvider,
) -> Result<markymark_mcp::RuntimeEngine> {
    match provider {
        SemanticProvider::Voyage => bail!(
            "--semantic-search voyage requires compiling with --features semantic-search,voyage"
        ),
        SemanticProvider::Local => bail!(
            "--semantic-search local requires compiling with --features semantic-search,local-embeddings"
        ),
        SemanticProvider::Hash => {
            bail!("--semantic-search hash requires compiling with --features semantic-search")
        }
    }
}

#[cfg(all(feature = "semantic-search", feature = "voyage"))]
fn build_voyage_provider() -> Result<Arc<dyn markymark_core::prelude::EmbeddingProvider>> {
    use markymark_core::embeddings::voyage::{VoyageConfig, VoyageProvider};

    let api_key = std::env::var("VOYAGE_API_KEY").unwrap_or_default();
    if api_key.trim().is_empty() {
        bail!("--semantic-search voyage requires the VOYAGE_API_KEY environment variable");
    }
    let provider = VoyageProvider::new(api_key, VoyageConfig::default())
        .map_err(|e| anyhow::anyhow!("failed to create Voyage provider: {e}"))?;
    Ok(Arc::new(provider))
}

#[cfg(all(feature = "semantic-search", not(feature = "voyage")))]
fn build_voyage_provider() -> Result<Arc<dyn markymark_core::prelude::EmbeddingProvider>> {
    bail!("--semantic-search voyage requires compiling with --features voyage")
}

#[cfg(all(feature = "semantic-search", feature = "local-embeddings"))]
fn build_local_provider() -> Result<Arc<dyn markymark_core::prelude::EmbeddingProvider>> {
    use markymark_core::embeddings::local::LocalOnnxProvider;

    let provider = LocalOnnxProvider::new(None)
        .map_err(|e| anyhow::anyhow!("failed to create local embedding provider: {e}"))?;
    Ok(Arc::new(provider))
}

#[cfg(all(feature = "semantic-search", not(feature = "local-embeddings")))]
fn build_local_provider() -> Result<Arc<dyn markymark_core::prelude::EmbeddingProvider>> {
    bail!("--semantic-search local requires compiling with --features local-embeddings")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parses_semantic_search_voyage() {
        let cli = Cli::try_parse_from(["markymark", "--mcp", "--semantic-search", "voyage", "."])
            .unwrap();
        assert!(cli.mcp);
        assert!(matches!(cli.semantic_search, Some(SemanticProvider::Voyage)));
    }

    #[test]
    fn cli_parses_semantic_search_hash() {
        let cli = Cli::try_parse_from(["markymark", "--mcp", "--semantic-search", "hash", "."])
            .unwrap();
        assert!(matches!(cli.semantic_search, Some(SemanticProvider::Hash)));
    }

    #[test]
    fn cli_no_semantic_search_default_none() {
        let cli = Cli::try_parse_from(["markymark", "--mcp", "."]).unwrap();
        assert!(cli.semantic_search.is_none());
    }

    #[test]
    fn cli_parses_semantic_search_local() {
        let cli = Cli::try_parse_from(["markymark", "--mcp", "--semantic-search", "local", "."])
            .unwrap();
        assert!(matches!(cli.semantic_search, Some(SemanticProvider::Local)));
    }

    #[test]
    fn cli_invalid_provider_rejected() {
        let result =
            Cli::try_parse_from(["markymark", "--mcp", "--semantic-search", "openai", "."]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_semantic_search_allowed_in_lsp_mode() {
        // Parsed OK — the warning is printed at runtime, not rejected by clap.
        let cli =
            Cli::try_parse_from(["markymark", "--lsp", "--semantic-search", "voyage"]).unwrap();
        assert!(cli.lsp);
        assert!(cli.semantic_search.is_some());
    }

    /// When semantic-search feature is not compiled, provider construction should fail
    /// with a clear error message rather than silently ignoring the flag.
    #[cfg(not(feature = "semantic-search"))]
    #[tokio::test]
    async fn build_engine_without_feature_returns_error() {
        let tmp = std::env::temp_dir().join("markymark-test-no-feature");
        let _ = std::fs::create_dir_all(&tmp);
        let result = build_engine_with_provider(vec![tmp], SemanticProvider::Hash).await;
        let err = result.err().expect("should fail without semantic-search feature");
        assert!(
            err.to_string().contains("--features semantic-search"),
            "error should mention the required feature flag, got: {err}"
        );
    }

    /// When semantic-search feature is compiled but local-embeddings is not,
    /// provider construction should fail with a clear feature flag message.
    #[cfg(all(feature = "semantic-search", not(feature = "local-embeddings")))]
    #[tokio::test]
    async fn build_local_without_feature_returns_error() {
        let tmp = std::env::temp_dir().join("markymark-test-no-local");
        let _ = std::fs::create_dir_all(&tmp);
        let result = build_engine_with_provider(vec![tmp], SemanticProvider::Local).await;
        let err = result.err().expect("should fail without local-embeddings feature");
        assert!(
            err.to_string().contains("--features local-embeddings"),
            "error should mention the required feature flag, got: {err}"
        );
    }

    /// When semantic-search + voyage features are compiled, missing VOYAGE_API_KEY
    /// should produce a clear error.
    #[cfg(all(feature = "semantic-search", feature = "voyage"))]
    #[test]
    fn voyage_missing_api_key_returns_error() {
        // Temporarily ensure VOYAGE_API_KEY is unset for this test.
        let saved = std::env::var("VOYAGE_API_KEY").ok();
        std::env::remove_var("VOYAGE_API_KEY");

        let result = build_voyage_provider();
        let err = result.err().expect("should fail without VOYAGE_API_KEY");
        assert!(
            err.to_string().contains("VOYAGE_API_KEY"),
            "error should mention VOYAGE_API_KEY, got: {err}"
        );

        // Restore if it was set.
        if let Some(val) = saved {
            std::env::set_var("VOYAGE_API_KEY", val);
        }
    }
}
