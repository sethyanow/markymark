#!/usr/bin/env bash
# smoke-local.sh — Quick smoke test for local ONNX embedding provider via MCP.
#
# Usage:
#   ./scripts/smoke-local.sh              # uses a small test workspace
#   ./scripts/smoke-local.sh /path/to/dir # custom workspace root
#
# Requires: local-embeddings feature compiled in (downloads model on first run).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "==> Building with semantic-search + local-embeddings features..."
cargo build -p markymark-cli --features semantic-search,local-embeddings 2>&1 | tail -3

BINARY="$REPO_ROOT/target/debug/markymark"

# --------------------------------------------------------------------------
# Create a small test workspace (avoids embedding 100+ docs at startup)
# --------------------------------------------------------------------------
TMPDIR_SMOKE=$(mktemp -d)
trap 'rm -rf "$TMPDIR_SMOKE"' EXIT

WORKSPACE_ROOT="${1:-$TMPDIR_SMOKE/workspace}"
if [[ "$WORKSPACE_ROOT" == "$TMPDIR_SMOKE/workspace" ]]; then
  mkdir -p "$WORKSPACE_ROOT"
  cat > "$WORKSPACE_ROOT/README.md" <<'HEREDOC'
# Test Workspace

This is a test document for the local ONNX embedding smoke test.

## Features

- Semantic search via local all-MiniLM-L6-v2 embeddings
- Real-time document indexing
HEREDOC
  cat > "$WORKSPACE_ROOT/architecture.md" <<'HEREDOC'
# Architecture

The embedding provider trait abstracts over different vector backends.

## Providers

- **Local**: ONNX inference with fastembed-rs for offline embeddings
- **Hash**: Deterministic local provider for testing
HEREDOC
  echo "  (using built-in 2-file test workspace)"
fi

echo "==> Workspace root: $WORKSPACE_ROOT"
echo "==> Starting MCP server (Local ONNX provider)..."

# --------------------------------------------------------------------------
# Set up a named pipe for controlled input
# --------------------------------------------------------------------------
FIFO="$TMPDIR_SMOKE/mcp_in"
TMPOUT="$TMPDIR_SMOKE/mcp_stdout"
TMPERR="$TMPDIR_SMOKE/mcp_stderr"
mkfifo "$FIFO"

# Start MCP server: stdin from FIFO, stdout/stderr separate
"$BINARY" --mcp --semantic-search local "$WORKSPACE_ROOT" \
  < "$FIFO" > "$TMPOUT" 2>"$TMPERR" &
MCP_PID=$!

# Open FIFO for writing (keeps server stdin open)
exec 3>"$FIFO"

send() {
  echo "  -> $1"
  echo "$2" >&3
}

# Give server time to index (local model may download on first run)
echo "  (waiting for startup + indexing... first run downloads ~23MB model)"
sleep 15

send "initialize" \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke-test","version":"0.1.0"}}}'
sleep 1

send "initialized notification" \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}'
sleep 1

send "tools/list" \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
sleep 1

send "tools/call semantic-search" \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"semantic-search","arguments":{"query":"embedding provider","realm":"default"}}}'
sleep 5

# Close write end — server sees EOF and shuts down
exec 3>&-

# Wait for exit (up to 10s)
for _ in $(seq 1 10); do
  kill -0 "$MCP_PID" 2>/dev/null || break
  sleep 1
done
kill "$MCP_PID" 2>/dev/null || true
wait "$MCP_PID" 2>/dev/null || true

# --------------------------------------------------------------------------
# Output
# --------------------------------------------------------------------------
echo ""
echo "==> Server stderr:"
cat "$TMPERR"
echo ""
echo "==> MCP responses:"
# Pretty-print to terminal, keep raw file for assertions
if command -v python3 &>/dev/null; then
  while IFS= read -r line; do
    echo "$line" | python3 -m json.tool 2>/dev/null || echo "$line"
  done < "$TMPOUT"
else
  cat "$TMPOUT"
fi

# --------------------------------------------------------------------------
# Assertions
# --------------------------------------------------------------------------
PASS=0
FAIL=0

check() {
  local label="$1" pattern="$2"
  if grep -q "$pattern" "$TMPOUT"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern: $pattern)"
    FAIL=$((FAIL + 1))
  fi
}

echo ""
echo "==> Assertions:"
check "Server initialized"          '"protocolVersion"'
check "Tools list returned"         '"tools"'
check "semantic-search tool exists"  'semantic-search'
check "Search returned result"      '"result"'

echo ""
echo "==> Results: $PASS passed, $FAIL failed"

if [[ $FAIL -gt 0 ]]; then
  echo "SMOKE TEST FAILED"
  exit 1
fi

echo "SMOKE TEST PASSED"
