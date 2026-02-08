//! markymark CLI entry point
//!
//! Supports both LSP and MCP transport modes.

#![warn(missing_docs)]
#![warn(clippy::all)]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("markymark: High-Performance Markdown LSP");
    println!("CLI will be implemented in Phase 6");
    Ok(())
}
