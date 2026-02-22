---
title: VS Code
description: Installing and configuring the markymark VS Code extension
---

The VS Code extension connects to the markymark language server automatically,
giving you diagnostics, navigation, and refactoring for `.md` and `.mdx` files
with no manual server configuration.

## Installation

The extension is not yet published to the VS Code Marketplace. Install it
manually from a VSIX file.

**Build the VSIX** (requires the markymark repo):

```bash
cd markymark-vscode
bun install && bun run compile
bunx @vscode/vsce package
```

**Install in VS Code:**

1. Open the Command Palette (Ctrl+Shift+P / Cmd+Shift+P)
2. Run **Extensions: Install from VSIX...**
3. Select the `.vsix` file produced by the previous step

The extension activates automatically when you open a Markdown or MDX file.

## Configuration

The extension has one setting:

| Setting | Default | Description |
|---------|---------|-------------|
| `markymark.path` | `""` (empty) | Path to the markymark binary. Leave empty to use the bundled binary. |

If you installed markymark to a non-standard location or want to use a development
build, set `markymark.path` in your VS Code settings to the full path of the binary.

For VS Code Remote (SSH, WSL, Containers), the extension runs on the remote
machine. If no bundled binary matches, set `markymark.path` to a manually
installed binary in your remote workspace settings.

## Features

Once active, these features work on any `.md` or `.mdx` file in your workspace:

- **Diagnostics** — broken links and duplicate headings appear as warnings in the editor
- **Go to definition** — Ctrl+Click (Cmd+Click) a wiki link or heading reference to jump to it
- **Find all references** — right-click a heading, select "Find All References"
- **Rename** — right-click a heading, select "Rename Symbol" (F2) to update all references
- **Outline** — Ctrl+Shift+O (Cmd+Shift+O) shows a hierarchical heading tree for the file
- **Workspace symbol search** — Ctrl+T (Cmd+T) to search headings across all files
- **Completion** — type `[[` to auto-complete wiki links; type `#` after a link to complete anchors
- **Hover** — hover over a heading to see backlink count and referring documents

## Tips

- markymark works on a **folder**, not single files. Open a folder or workspace to
  enable cross-file features like navigation and reference detection.
- Multi-root workspaces are supported — each root folder becomes a separate realm.
- The extension requires VS Code 1.85 or later.
