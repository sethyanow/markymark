//! markymark-mcp stdio entrypoint.

#![warn(clippy::all)]

use std::sync::Arc;

use markymark_mcp::{run_stdio, RuntimeEngine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let workspace_roots = if std::env::args().len() > 1 {
        std::env::args()
            .skip(1)
            .map(std::path::PathBuf::from)
            .collect()
    } else {
        vec![std::env::current_dir()?]
    };

    let engine = RuntimeEngine::from_workspace_roots(workspace_roots).await?;
    run_stdio(Arc::new(engine)).await
}
