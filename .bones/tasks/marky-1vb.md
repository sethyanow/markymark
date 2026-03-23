---
id: marky-1vb
title: Integrate plugin hooks into markymark-plugin
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-peu
---



Move the suggest-lsp PreToolUse hook from examples/ into the actual plugin, and add a SessionStart hook for context loading.

## Deliverables
1. markymark-plugin/hooks/hooks.json with PreToolUse and SessionStart hooks
2. markymark-plugin/hooks/suggest-lsp.sh (moved from examples, adapted paths)
3. Tests for the integrated hooks

## Design

## Goal
Move the suggest-lsp PreToolUse hook from examples/ into the actual plugin and add tests.

## Codebase Verification
- markymark-plugin/.claude-plugin/plugin.json exists with LSP/MCP config
- examples/claude-code-plugin/hooks/suggest-lsp.sh exists and works (9 tests pass)
- No markymark-plugin/hooks/ directory exists — needs creation
- ${CLAUDE_PLUGIN_ROOT} is used in existing scripts (select-binary.sh)
- Existing test suite at markymark-plugin/tests/test_select_binary.sh (338 lines, 9 tests)

## Implementation Steps

### Step 1: Create hooks directory and copy suggest-lsp.sh
\`\`\`bash
mkdir -p markymark-plugin/hooks
cp examples/claude-code-plugin/hooks/suggest-lsp.sh markymark-plugin/hooks/suggest-lsp.sh
chmod +x markymark-plugin/hooks/suggest-lsp.sh
\`\`\`

### Step 2: Create hooks.json with PreToolUse hook
Create markymark-plugin/hooks/hooks.json:
\`\`\`json
{
  "description": "markymark LSP-first workflow hooks",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read",
        "hooks": [
          {
            "type": "command",
            "command": "bash \${CLAUDE_PLUGIN_ROOT}/hooks/suggest-lsp.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
\`\`\`

### Step 3: Write tests for integrated hooks
Create markymark-plugin/tests/test_hooks.sh covering:
- hooks.json is valid JSON
- hooks.json has correct structure (description + hooks wrapper)
- suggest-lsp.sh exists and is executable
- suggest-lsp.sh returns valid JSON for .md input
- suggest-lsp.sh returns empty for non-.md input
- \${CLAUDE_PLUGIN_ROOT} used in command paths (not hardcoded)

### Step 4: Run all tests
\`\`\`bash
bash markymark-plugin/tests/test_select_binary.sh
bash markymark-plugin/tests/test_hooks.sh
\`\`\`

### Step 5: Commit
\`\`\`bash
git add markymark-plugin/hooks/
git commit -m "feat(plugin): integrate LSP-first PreToolUse hook"
\`\`\`

## Success Criteria
- [ ] markymark-plugin/hooks/hooks.json exists with PreToolUse matcher
- [ ] markymark-plugin/hooks/suggest-lsp.sh works for .md and .mdx
- [ ] All new and existing tests pass
