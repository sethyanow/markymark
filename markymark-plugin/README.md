# markymark - Claude Code Plugin

Markdown intelligence for AI assistants and code editors. LSP and MCP server supporting wiki links, headings, XML tags, and cross-document navigation.

## Features

### LSP (Language Server Protocol)
- **Go-to-definition**: Jump to wiki link targets, markdown anchors, XML tag declarations
- **Find references**: Find all usages of headings, wiki links, and XML tags
- **Hover**: Rich hover info for headings (backlinks, usage stats) and XML tags (workspace stats, common attributes)
- **Completion**: Auto-complete wiki links, markdown links, and XML tag names
- **Rename**: Rename headings and update all references (wiki links + markdown anchors)
- **Diagnostics**: Real-time validation of broken links, duplicate headings, unclosed XML tags
- **Document symbols**: Hierarchical outline of headings and XML tags

### MCP (Model Context Protocol)
- **get-outline**: Extract document outline with heading hierarchy
- **search-symbols**: Find headings and XML tags across workspace
- **find-references**: Locate all references to a symbol
- **rename**: Rename symbols with workspace-wide updates
- **Realm management**: Manage multiple workspace realms for cross-project navigation

### Plugin Skills
- **/markdown-check**: Validate markdown quality across workspace (broken links, duplicate headings, XML issues)

## Installation

### Method 1: Claude Code Plugin (Recommended)

**Via Marketplace** (when published):
```bash
# From Claude Code
/plugin install markymark
```

**Manual Installation**:
```bash
# Download latest release
gh release download --repo sethyanow/markdown-mcp --pattern 'markymark-plugin-*.tar.gz'

# Extract and install
tar -xzf markymark-plugin-*.tar.gz
claude-code --plugin-dir ./markymark-plugin
```

### Method 2: Cargo Install

Install the binary directly via cargo:

```bash
cargo install markymark-cli
```

This installs the `markymark` binary to `~/.cargo/bin/`. You can then:

**Use as LSP server**:
```bash
markymark --lsp
```

**Use as MCP server**:
```bash
markymark --mcp
```

### Method 3: GitHub Releases

Download pre-built binaries for your platform:

1. Go to [Releases](https://github.com/sethyanow/markdown-mcp/releases)
2. Download the binary for your platform:
   - macOS ARM: `markymark-aarch64-apple-darwin`
   - macOS Intel: `markymark-x86_64-apple-darwin`
   - Linux x86_64: `markymark-x86_64-unknown-linux-gnu`
   - Linux ARM64: `markymark-aarch64-unknown-linux-gnu`
   - Windows: `markymark-x86_64-pc-windows-msvc.exe`
3. Make executable: `chmod +x markymark-*`
4. Move to PATH: `mv markymark-* /usr/local/bin/markymark`

## Configuration

### LSP Configuration

The plugin automatically configures LSP for `**/*.md` and `**/*.mdx` files. Manual configuration:

```json
{
  "command": "${CLAUDE_PLUGIN_ROOT}/bin/markymark",
  "args": ["--lsp"],
  "rootPatterns": [".git", "package.json", "Cargo.toml"],
  "filePatterns": ["**/*.md", "**/*.mdx"]
}
```

### MCP Configuration

The plugin automatically configures MCP tools. Manual configuration:

```json
{
  "command": "${CLAUDE_PLUGIN_ROOT}/bin/markymark",
  "args": ["--mcp", "${WORKSPACE_ROOT}"],
  "transport": "stdio"
}
```

## Usage

### In Claude Code

Once installed, markymark features work automatically:

- Open any `.md` or `.mdx` file
- LSP features (go-to-definition, hover, etc.) work instantly
- Use MCP tools in conversations:
  ```
  Use get-outline to show me the structure of docs/architecture.md
  ```
- Run skills:
  ```
  /markdown-check
  ```

### As Standalone LSP

```bash
# Start LSP server
markymark --lsp

# Server listens on stdio
# Configure your editor to use markymark as markdown LSP
```

### As Standalone MCP

```bash
# Start MCP server for a workspace
markymark --mcp /path/to/workspace

# Server provides MCP tools over stdio
```

## Platform Support

Pre-built binaries provided for:
- macOS ARM64 (Apple Silicon)
- macOS x86_64 (Intel)
- Linux x86_64
- Linux ARM64 (aarch64)
- Windows x86_64

The plugin includes platform detection that automatically selects the correct binary.

## Building from Source

```bash
# Clone repo
git clone https://github.com/sethyanow/markdown-mcp.git
cd markdown-mcp/markymark

# Build
cargo build --release

# Binary at target/release/markymark
```

## Supported Markdown Flavors

- **Obsidian**: Wiki links `[[page]]`, callouts, block IDs `^id`, embeds `![[file]]`
- **Logseq**: Nested lists, block UUIDs, page properties
- **CommonMark**: Standard markdown with heading anchors

## Development

See the main repo [README](https://github.com/sethyanow/markdown-mcp) for development setup.

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/sethyanow/markdown-mcp)
- [Issue Tracker](https://github.com/sethyanow/markdown-mcp/issues)
- [Documentation](https://github.com/sethyanow/markdown-mcp/tree/main/docs)
