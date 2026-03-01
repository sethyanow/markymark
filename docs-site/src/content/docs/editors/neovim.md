---
title: Neovim
description: Configuring markymark as a language server in Neovim
---

Neovim's built-in LSP client connects directly to the markymark binary, giving
you diagnostics, navigation, and refactoring for Markdown files without any
additional plugins beyond a small config snippet.

## Prerequisites

The markymark binary must be installed and available on your PATH. See the
[installation guide](/getting-started/installation/) for instructions on
installing via cargo or pre-built binaries.

Verify the binary is available:

```bash
markymark --version
```

## Configuration

Add this to your Neovim config (typically `~/.config/nvim/init.lua` or a
file sourced from it):

```lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'markdown',
  callback = function()
    vim.lsp.start({
      name = 'markymark',
      cmd = { 'markymark', '--lsp' },
      root_dir = vim.fs.dirname(
        vim.fs.find({ '.git', 'package.json', 'Cargo.toml' }, { upward = true })[1]
      ),
    })
  end,
})
```

This starts the markymark language server whenever you open a Markdown file. The
`root_dir` determines the workspace boundary — all `.md` files under that
directory are indexed together.

## Features

With the LSP client connected, you get:

- **Diagnostics** — broken links and duplicate headings appear inline
- **Go to definition** — jump to wiki link targets and heading anchors with `gd` or `:lua vim.lsp.buf.definition()`
- **Find references** — `:lua vim.lsp.buf.references()` shows all documents linking to a heading
- **Rename** — `:lua vim.lsp.buf.rename()` renames a heading and updates all references workspace-wide
- **Hover** — `K` shows backlink count and referring documents for a heading
- **Document symbols** — `:lua vim.lsp.buf.document_symbol()` lists headings in the current file
- **Workspace symbol search** — `:lua vim.lsp.buf.workspace_symbol('')` searches headings across all files

### Telescope integration

If you use [Telescope](https://github.com/nvim-telescope/telescope.nvim), workspace
symbols are available via:

```vim
:Telescope lsp_workspace_symbols
```

This gives you fuzzy search over all headings and tags across your workspace.

## Tips

- markymark indexes the entire directory tree under `root_dir`. If you want to
  limit scope, set `root_dir` to a more specific folder.
- For MDX files, add `'mdx'` to the `pattern` field in the autocommand.
- If the server does not start, run `:LspLog` to check for errors.
