# Markymark — VSCode Extension

Markdown LSP intelligence powered by [markymark](https://github.com/sethyanow/markymark):
hover, go-to-definition, find-references, completions, and diagnostics for `.md` and `.mdx` files.

## Features

- **Hover**: Show link targets and heading details
- **Find References**: All files that link to the current heading or file
- **Go to Definition**: Follow wiki links and markdown links
- **Completion**: Complete wiki links, heading anchors, and tags
- **Diagnostics**: Broken links, duplicate headings
- **Frontmatter awareness**: Obsidian aliases, Logseq properties

## Requirements

The extension bundles the `markymark` binary for:

| Platform | Architecture |
|----------|-------------|
| macOS    | Apple Silicon (arm64), Intel (x64) |
| Linux    | x64, arm64 |
| Windows  | x64 (arm64 via emulation) |

If your platform is not listed, install `markymark` on your PATH or configure `markymark.path`.

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `markymark.path` | `""` | Path to markymark binary. If empty, uses the bundled binary. |

## Remote Development (SSH, WSL, Containers)

This extension runs on the **remote machine**. The binary must match the remote platform.
The bundled binary is selected automatically based on the remote platform and architecture.

If no bundled binary matches, set `markymark.path` in your remote `.vscode/settings.json`
pointing to a manually installed `markymark` binary.
