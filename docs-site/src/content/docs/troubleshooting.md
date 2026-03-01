---
title: Troubleshooting
description: Solutions to common markymark issues and how to get help
---

## LSP not starting

**No diagnostics, no hover, no symbol search in your editor.**

Check that the language server binary is on your PATH:

```bash
markymark --version
```

If the command is not found, see the [installation guide](/getting-started/installation/).

For VS Code, the extension bundles its own binary — reinstall the extension if it
stops working. For Neovim, verify your `cmd` path in `lspconfig` points to the
correct binary.

## MCP server not connecting

**Agent tools unavailable or connection errors.**

markymark's MCP server uses stdio transport (stdin/stdout), not HTTP. Start it with:

```bash
markymark --mcp /path/to/workspace
```

The workspace path argument is required — without it, the server has no files to
index. If using the Claude Code plugin, the plugin handles server startup
automatically.

Common causes:
- **Wrong binary path** in your MCP client configuration
- **Missing workspace argument** — the `--mcp` flag needs a directory path
- **Binary not executable** — run `chmod +x` on the downloaded binary

## Workspace not indexing files

**Files exist but don't appear in search or diagnostics.**

markymark indexes files based on extension. Supported extensions: `.md`,
`.markdown`, `.json`, `.jsonc`, `.json5`, `.jsonl`, `.yaml`, `.yml`, `.toml`,
`.env`, `.ini`, `.cfg`.

If your files have non-standard extensions, they won't be detected. Rename them
or create a symlink with a recognized extension.

For MCP mode, files must be under a workspace root added via `add-root`. Check
your realm's roots with `realm-stats`.

## Build failures from source

**Compilation errors when running `cargo build`.**

markymark requires both Rust (1.80+) and Zig (0.15.2+) toolchains. The Zig
toolchain compiles the Zig FFI layer during the Rust build.

```bash
rustc --version   # needs 1.80+
zig version       # needs 0.15.2+
```

If Zig is missing or the wrong version, the build fails during the
`markymark-kernels` crate compilation. Install Zig 0.15.2+ from
[ziglang.org/download](https://ziglang.org/download/).

## Performance on large workspaces

markymark is designed for fast response times. If you notice lag:

- **Initial indexing** of thousands of files takes a moment on first open.
  Subsequent edits are incremental and fast (sub-5ms for typical documents).
- **Very large files** (100KB+ markdown) take longer to parse. This is rare in
  practice.
- **Structured format indexing** (JSON, YAML) is lighter than markdown since
  fewer features apply.

## Filing a bug report

If your issue isn't covered above, file a report at
[github.com/sethyanow/markymark/issues](https://github.com/sethyanow/markymark/issues).

Include:
- markymark version (`markymark --version`)
- Editor and extension version (if using LSP)
- A minimal file or workspace that reproduces the issue
- The error message or unexpected behavior you observed
