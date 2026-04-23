"""Workspace version — single source of truth for Bazel targets.

Loaded by BUILD.bazel files to set `rust_binary(version=...)` /
`rust_library(version=...)`, which drives `CARGO_PKG_VERSION` at compile time.

Keep in sync with `Cargo.toml`, `MODULE.bazel`, and
`markymark-plugin/.claude-plugin/plugin.json`. See RELEASING.md.
"""

VERSION = "0.8.0-dev0"
