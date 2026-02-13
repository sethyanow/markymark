#!/bin/bash
# suggest-lsp.sh — PreToolUse hook for Read on markdown files
#
# Suggests using markymark LSP (documentSymbol, hover, diagnostics)
# before reading the full file. Does NOT block the Read — just adds
# a system message with the suggestion.
#
# Usage: Receives JSON on stdin from Claude Code PreToolUse event.
set -euo pipefail

input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only act on markdown files
case "$file_path" in
  *.md|*.mdx) ;;
  *) exit 0 ;;
esac

# Suggest LSP-first workflow without blocking
cat <<'EOF'
{
  "hookSpecificOutput": {
    "permissionDecision": "allow"
  },
  "systemMessage": "Tip: markymark LSP is available for this markdown file. Consider using LSP documentSymbol to get the heading/XML outline (~95% fewer tokens than a full Read). Use LSP hover on headings for backlink info, or LSP findReferences to locate all usages. Diagnostics for broken links and duplicate headings are reported automatically. Only use Read if you need the full prose content."
}
EOF
