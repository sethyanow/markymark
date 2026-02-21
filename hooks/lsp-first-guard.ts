#!/usr/bin/env bun
// hooks/lsp-first-guard.ts
// PreToolUse hook for Read: warns on full-file reads of large code files.
// Exit 0 always (warn only, never block). stdout = message injected into context.

const CODE_EXTENSIONS = new Set([
  ".rs",
  ".zig",
  ".ts",
  ".tsx",
  ".py",
  ".go",
  ".c",
  ".cpp",
  ".h",
  ".hpp",
  ".java",
]);

// Files under this size (bytes) are assumed <100 lines and skipped.
// Heuristic: code averages ~25-30 bytes/line for well-formatted code.
// 200 lines * 40 = 8000. Threshold set to catch files >200 lines to avoid
// false positives on borderline files while catching the real offenders (500+ lines).
const SIZE_THRESHOLD = 8000;

interface PreToolUsePayload {
  session_id: string;
  tool_name: string;
  tool_input: Record<string, unknown>;
}

try {
  const raw = await Bun.stdin.text();
  if (!raw.trim()) process.exit(0);

  const payload: PreToolUsePayload = JSON.parse(raw);
  if (payload.tool_name !== "Read") process.exit(0);

  const filePath = payload.tool_input?.file_path;
  if (typeof filePath !== "string") process.exit(0);

  // Targeted reads (offset OR limit specified) are fine
  if (payload.tool_input?.offset != null || payload.tool_input?.limit != null) {
    process.exit(0);
  }

  // Only check code files by extension
  const dotIdx = filePath.lastIndexOf(".");
  if (dotIdx === -1) process.exit(0);
  const ext = filePath.substring(dotIdx);
  if (!CODE_EXTENSIONS.has(ext)) process.exit(0);

  // Check file exists and exceeds size threshold
  const file = Bun.file(filePath);
  if (!(await file.exists())) process.exit(0);
  if (file.size < SIZE_THRESHOLD) process.exit(0);

  const basename = filePath.split("/").pop() ?? filePath;
  const approxLines = Math.round(file.size / 40);
  console.log(
    `\u26a0\ufe0f LSP-FIRST (Rule 9): Full read of ${basename} (~${approxLines} lines) without offset/limit.`,
  );
  console.log(
    "Use LSP documentSymbol first, then Read with offset+limit.",
  );
} catch {
  // Never crash - silent exit on any error
}
process.exit(0);
