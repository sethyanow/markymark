---
id: marky-10h
title: Add Claude Code integration guide for LSP-first markdown reading
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

Create documentation and example plugin hook to encourage LSP usage before reading full markdown files.

**Context:**
AI agents using Claude Code can save significant tokens by querying markymark LSP for structure/diagnostics before reading entire markdown files. For a 260-line file, LSP queries use ~100 tokens vs ~2000+ for full reads (~95% savings).

**Deliverables:**

1. **Documentation in README.md:**
   - Section on Claude Code integration
   - Explain LSP-first workflow benefits
   - Example LSP queries (documentSymbol, hover, diagnostics)
   - Token savings analysis

2. **Example Claude Code plugin hook:**
   Create `examples/claude-code-plugin/` with PreToolUse hook that:
   - Intercepts Read tool calls on `*.md` files
   - Suggests trying LSP operations first
   - Shows example: `LSP documentSymbol` before Read
   - Provides opt-out for cases requiring full content

3. **CLAUDE.md template snippet:**
   Provide copy-paste rule for user CLAUDE.md files:
   ```markdown
   ## Markdown Intelligence
   
   ALWAYS use markymark LSP before reading markdown files:
   - `LSP documentSymbol` for structure/outline
   - Check automatic diagnostics for broken links/duplicates
   - Only use Read tool if you need full content
   ```

**Benefits:**
- Reduces token usage for markdown exploration
- Better user experience (faster responses)
- Demonstrates LSP value to Claude Code users
- Provides plugin development example
