---
id: marky-hxwf
title: Add LSP-first PreToolUse hook for Read enforcement
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

Add a PreToolUse hook that warns when full-file Read is called on code files >100 lines without offset/limit. Also add Project Rule #9 to CLAUDE.md.

## Design

## Goal
Add a PreToolUse hook that warns when the Read tool is called on code files (>100 lines)
without offset/limit, enforcing the LSP-first workflow. Add Project Rule #9 to CLAUDE.md.

## Effort Estimate
1-2 hours across 3 files.

## Implementation

### 1. Hook script: hooks/lsp-first-guard.ts

TypeScript (Bun), following security-validator.ts pattern. Location: project-local hooks/ dir
(matches existing session-end.sh convention, referenced via \$CLAUDE_PROJECT_DIR/hooks/).

```typescript
#!/usr/bin/env bun
// hooks/lsp-first-guard.ts
// PreToolUse hook for Read: warns on full-file reads of large code files.
// Exit 0 always (warn only, never block). stdout = message injected into context.

const CODE_EXTENSIONS = new Set([
  '.rs', '.zig', '.ts', '.tsx', '.py', '.go', '.c', '.cpp', '.h', '.hpp', '.java',
]);
const LINE_THRESHOLD = 100;

interface PreToolUsePayload {
  session_id: string;
  tool_name: string;
  tool_input: Record<string, any>;
}

try {
  const raw = await Bun.stdin.text();
  if (!raw.trim()) process.exit(0);

  const payload: PreToolUsePayload = JSON.parse(raw);
  if (payload.tool_name !== 'Read') process.exit(0);

  const filePath: string | undefined = payload.tool_input?.file_path;
  if (!filePath) process.exit(0);

  // Targeted reads (offset OR limit specified) are fine
  if (payload.tool_input?.offset != null || payload.tool_input?.limit != null) {
    process.exit(0);
  }

  // Only check code files
  const ext = filePath.substring(filePath.lastIndexOf('.'));
  if (!CODE_EXTENSIONS.has(ext)) process.exit(0);

  // Check file exists and line count exceeds threshold
  const file = Bun.file(filePath);
  if (!(await file.exists())) process.exit(0);

  // Use file size as line-count heuristic: ~40 bytes/line avg for code
  // 100 lines * 40 = 4000 bytes. Use 5000 for conservative margin.
  if (file.size < 5000) process.exit(0);

  const basename = filePath.split('/').pop() ?? filePath;
  const approxLines = Math.round(file.size / 40);
  console.log(
    \`⚠️ LSP-FIRST (Rule 9): Full read of \${basename} (~\${approxLines} lines) without offset/limit.\`
  );
  console.log('Use LSP documentSymbol first, then Read with offset+limit.');
} catch {
  // Never crash — silent exit on any error
}
process.exit(0);
```

### 2. Settings update: .claude/settings.local.json

Add to hooks.PreToolUse array:
```json
{
  "matcher": "Read",
  "hooks": [
    {
      "type": "command",
      "command": "\$CLAUDE_PROJECT_DIR/hooks/lsp-first-guard.ts"
    }
  ]
}
```

### 3. CLAUDE.md: Add Project Rule #9

Add row to Project Rules table after rule 8:
```
| 9 | **LSP-first: no unbounded Read on code files >100 lines** | Use LSP documentSymbol first, then Read with offset+limit. PreToolUse hook warns on violations. Full-file reads waste 5-25k tokens per file. |
```

## Success Criteria
- [ ] hooks/lsp-first-guard.ts exists and is executable (chmod +x)
- [ ] .claude/settings.local.json has PreToolUse entry for Read matcher
- [ ] CLAUDE.md has Rule #9 in Project Rules table
- [ ] Hook fires: Reading a .rs/.zig file >100 lines without offset/limit → stdout warning
- [ ] Hook silent: Reading same file WITH offset or limit → no output
- [ ] Hook silent: Reading .md/.json/.toml file of any size → no output
- [ ] Hook silent: Reading small code file (<100 lines) → no output
- [ ] Hook silent: File doesn't exist → no output, exit 0
- [ ] Hook safe: Malformed JSON stdin → no crash, exit 0
- [ ] Hook safe: Empty stdin → no crash, exit 0
- [ ] All existing hooks unaffected (session-end.sh, security-validator, etc.)
- [ ] Pre-commit hooks pass
- [ ] cargo nextest passes (no code changes to crate sources)

## Tests (Manual Verification)

```bash
# Test 1: Large code file, no offset/limit → should warn
echo '{"session_id":"test","tool_name":"Read","tool_input":{"file_path":"zig/src/md4c/extraction_renderer.zig"}}' | bun hooks/lsp-first-guard.ts
# Expected: ⚠️ LSP-FIRST message

# Test 2: Large code file WITH offset+limit → should be silent
echo '{"session_id":"test","tool_name":"Read","tool_input":{"file_path":"zig/src/md4c/extraction_renderer.zig","offset":1,"limit":50}}' | bun hooks/lsp-first-guard.ts
# Expected: no output

# Test 3: Markdown file → should be silent
echo '{"session_id":"test","tool_name":"Read","tool_input":{"file_path":"docs/MEMORY.md"}}' | bun hooks/lsp-first-guard.ts
# Expected: no output

# Test 4: Small code file → should be silent
echo '{"session_id":"test","tool_name":"Read","tool_input":{"file_path":"markymark-core/src/lib.rs"}}' | bun hooks/lsp-first-guard.ts
# Expected: no output (file is small)

# Test 5: Empty stdin → exit 0, no crash
echo '' | bun hooks/lsp-first-guard.ts
# Expected: no output, exit code 0

# Test 6: Malformed JSON → exit 0, no crash
echo 'not json' | bun hooks/lsp-first-guard.ts
# Expected: no output, exit code 0
```

## Key Considerations (SRE Review)

### Edge Case: offset without limit
Read with offset=100 but no limit reads from line 100 to default 2000 lines.
This is somewhat targeted. Decision: treat as targeted (allow). The user made a
deliberate choice to start at a specific line.

### Edge Case: limit without offset
Read with limit=50 but no offset reads first 50 lines. This is targeted. Allow.

### Edge Case: File size heuristic vs actual line count
Using file.size / 40 as line approximation. Code files average 30-60 bytes/line.
5KB threshold (100 lines × 50 bytes) is conservative — will not fire on files
that are genuinely ~100 lines. May miss files with very long lines (e.g., minified).
Acceptable tradeoff: false negatives (missing a warning) are harmless; false
positives (warning on small files) would be annoying.

### Edge Case: Symlinks
Bun.file() follows symlinks. docs/modules and docs/zig_agent_docs are symlinks
to the forge repo (per memory #32470). If those contain .rs/.zig files, the hook
fires based on the actual file size. This is correct behavior.

### Edge Case: Non-existent files
Read tool can be called on files that don't exist (returns error). Hook checks
file.exists() and exits silently if false.

### Performance
Hook adds ~20ms to each Read call (Bun startup + stat). Acceptable for a
developer-facing tool. No file content is read — only Bun.file().size (stat call).

### Exit code 0 always
Hook MUST never exit with code 2 (block). This is advisory only. Blocking would
prevent legitimate full-file reads (e.g., before Edit tool which requires prior Read).
If enforcement needs to be stricter later, escalate to exit 2 as a separate decision.

## Anti-Patterns (FORBIDDEN)
- NO exit code 2 (block) — warn only, never block legitimate reads
- NO reading file contents in the hook — use file.size stat only
- NO complex logic (regex, AST) — keep the hook under 50 lines
- NO external dependencies beyond Bun builtins
- NO modifying tool_input or tool behavior — hook is observational only
- NO logging to files or external services from this hook (keep simple)
