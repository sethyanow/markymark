//! Shared test utilities for `markymark-mcp` integration tests.

// Not every consumer uses every method; suppress per-binary dead_code lint.
#![allow(dead_code)]

use std::path::PathBuf;

/// A temporary workspace directory that is automatically cleaned up when dropped.
pub struct TempWorkspace {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl TempWorkspace {
    pub fn new(name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("markymark-mcp-test-{name}-"))
            .tempdir()
            .expect("secure temporary workspace directory should be created");
        let root = dir.path().to_path_buf();
        Self { _dir: dir, root }
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn write(&self, name: &str, content: &str) {
        std::fs::write(self.root.join(name), content).expect("write file");
    }
}
