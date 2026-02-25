//! markymark CLI entry point.
//!
//! Supports both LSP and MCP transport modes via `--lsp` / `--mcp` flags.
//! Defaults to LSP when neither flag is specified.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

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
        run_mcp(roots).await
    } else {
        // Default to LSP mode (when neither flag or --lsp is specified).
        run_lsp().await
    }
}

async fn run_lsp() -> Result<()> {
    markymark_lsp::run_stdio().await;
    Ok(())
}

async fn run_mcp(roots: Vec<PathBuf>) -> Result<()> {
    let engine = markymark_mcp::RuntimeEngine::from_workspace_roots(roots).await?;
    markymark_mcp::run_stdio(Arc::new(engine)).await
}
