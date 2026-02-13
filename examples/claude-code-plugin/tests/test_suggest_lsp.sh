#!/bin/bash
# Tests for suggest-lsp.sh PreToolUse hook
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOOK="$SCRIPT_DIR/../hooks/suggest-lsp.sh"
PASS=0
FAIL=0

assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $desc"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $desc"
    echo "    expected: $expected"
    echo "    actual:   $actual"
    FAIL=$((FAIL + 1))
  fi
}

assert_json_field() {
  local desc="$1" json="$2" field="$3" expected="$4"
  local actual
  actual=$(echo "$json" | jq -r "$field")
  assert_eq "$desc" "$expected" "$actual"
}

echo "=== suggest-lsp.sh tests ==="

# --- Test 1: .md file returns allow with systemMessage ---
echo "Test 1: .md file triggers suggestion"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/docs/readme.md"}}' | bash "$HOOK")
assert_json_field "permissionDecision is allow" "$output" '.hookSpecificOutput.permissionDecision' "allow"
msg=$(echo "$output" | jq -r '.systemMessage')
if echo "$msg" | grep -q 'documentSymbol'; then
  assert_eq "systemMessage mentions documentSymbol" "yes" "yes"
else
  assert_eq "systemMessage mentions documentSymbol" "yes" "no"
fi

# --- Test 2: .mdx file triggers suggestion ---
echo "Test 2: .mdx file triggers suggestion"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/docs/page.mdx"}}' | bash "$HOOK")
assert_json_field "permissionDecision is allow" "$output" '.hookSpecificOutput.permissionDecision' "allow"

# --- Test 3: Non-markdown file produces no output ---
echo "Test 3: .rs file passes through silently"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/src/main.rs"}}' | bash "$HOOK")
assert_eq "no output for .rs file" "" "$output"

# --- Test 4: .txt file produces no output ---
echo "Test 4: .txt file passes through silently"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/notes.txt"}}' | bash "$HOOK")
assert_eq "no output for .txt file" "" "$output"

# --- Test 5: Missing file_path produces no output ---
echo "Test 5: missing file_path passes through silently"
output=$(echo '{"tool_name":"Read","tool_input":{}}' | bash "$HOOK")
assert_eq "no output for missing path" "" "$output"

# --- Test 6: Output is valid JSON ---
echo "Test 6: output is valid JSON"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"test.md"}}' | bash "$HOOK")
if echo "$output" | jq . > /dev/null 2>&1; then
  echo "  PASS: valid JSON"
  PASS=$((PASS + 1))
else
  echo "  FAIL: invalid JSON output"
  FAIL=$((FAIL + 1))
fi

# --- Test 7: Path with spaces ---
echo "Test 7: path with spaces"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/my docs/read me.md"}}' | bash "$HOOK")
assert_json_field "permissionDecision is allow" "$output" '.hookSpecificOutput.permissionDecision' "allow"

# --- Test 8: Nested path ---
echo "Test 8: deeply nested .md path"
output=$(echo '{"tool_name":"Read","tool_input":{"file_path":"/a/b/c/d/e/file.md"}}' | bash "$HOOK")
assert_json_field "permissionDecision is allow" "$output" '.hookSpecificOutput.permissionDecision' "allow"

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
