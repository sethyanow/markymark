# Example: LSP-First Markdown Reading Hook

A Claude Code plugin hook that suggests using markymark LSP before reading markdown files with the `Read` tool.

## What It Does

When Claude Code's `Read` tool is called on a `.md` or `.mdx` file, this hook adds a system message suggesting LSP-first alternatives:

- `LSP documentSymbol` for structure/outline (~95% fewer tokens)
- `LSP hover` for heading backlinks and XML tag stats
- `LSP findReferences` to locate all usages of a symbol
- Automatic diagnostics for broken links and duplicate headings

The hook **does not block** the Read — it allows it through and adds the suggestion as context.

## Installation

Copy this directory into your Claude Code plugin, or add the hook to your existing plugin's `hooks/hooks.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read",
        "hooks": [
          {
            "type": "command",
            "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/suggest-lsp.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

## Opt-Out

The hook only triggers on `.md` and `.mdx` files. All other file types pass through silently.

To disable the hook entirely, remove the `PreToolUse` entry from `hooks.json` and restart Claude Code.

## How It Works

1. Claude Code fires a `PreToolUse` event before running `Read`
2. The hook reads the tool input JSON from stdin
3. It checks if `file_path` ends in `.md` or `.mdx`
4. For markdown files: returns `permissionDecision: "allow"` with a `systemMessage` suggesting LSP
5. For other files: exits silently (no output, exit 0)
