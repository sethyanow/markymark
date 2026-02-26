#!/usr/bin/env bash
# smoke-voyage.sh — Quick smoke test for Voyage embedding provider via MCP.
#
# Usage:
#   ./scripts/smoke-voyage.sh              # uses a small test workspace
#   ./scripts/smoke-voyage.sh /path/to/dir # custom workspace root
#
# Requires: VOYAGE_API_KEY in environment (or .env in repo root).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# --------------------------------------------------------------------------
# Load .env if present and VOYAGE_API_KEY not already set
# --------------------------------------------------------------------------
if [[ -z "${VOYAGE_API_KEY:-}" && -f "$REPO_ROOT/.env" ]]; then
  # shellcheck source=/dev/null
  set -a; source "$REPO_ROOT/.env"; set +a
fi

if [[ -z "${VOYAGE_API_KEY:-}" ]]; then
  echo "ERROR: VOYAGE_API_KEY not set. Export it or add to $REPO_ROOT/.env" >&2
  exit 1
fi

echo "==> Building with semantic-search + voyage features..."
cargo build -p markymark-cli --features semantic-search,voyage 2>&1 | tail -3

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

This is a test document for the Voyage embedding smoke test.

## Features

- Semantic search via Voyage AI embeddings
- Real-time document indexing
HEREDOC
  cat > "$WORKSPACE_ROOT/architecture.md" <<'HEREDOC'
# Architecture

The embedding provider trait abstracts over different vector backends.

## Providers

- **Voyage**: Cloud API for production-quality embeddings
- **Hash**: Deterministic local provider for testing
HEREDOC
  echo "  (using built-in 2-file test workspace)"
else
  if [[ ! -d "$WORKSPACE_ROOT" ]]; then
    echo "ERROR: workspace root does not exist or is not a directory: $WORKSPACE_ROOT" >&2
    exit 1
  fi
fi

echo "==> Workspace root: $WORKSPACE_ROOT"
echo "==> Starting MCP server (Voyage provider)..."

# --------------------------------------------------------------------------
# Set up a named pipe for controlled input
# --------------------------------------------------------------------------
FIFO="$TMPDIR_SMOKE/mcp_in"
TMPOUT="$TMPDIR_SMOKE/mcp_stdout"
TMPERR="$TMPDIR_SMOKE/mcp_stderr"
mkfifo "$FIFO"

# Start MCP server: stdin from FIFO, stdout/stderr separate
"$BINARY" --mcp --semantic-search voyage "$WORKSPACE_ROOT" \
  < "$FIFO" > "$TMPOUT" 2>"$TMPERR" &
MCP_PID=$!

# Open FIFO for writing (keeps server stdin open)
exec 3>"$FIFO"

send() {
  echo "  -> $1"
  echo "$2" >&3
}

# Give server time to index (Voyage API calls for each doc)
echo "  (waiting for startup + indexing...)"
sleep 8

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
check "Initialize response (id=1)"            '"id":1'
check "Tools/list response (id=2)"            '"id":2'
check "semantic-search tool advertised"        'semantic-search'
check "semantic-search response (id=3)"        '"id":3'

echo ""
echo "==> Results: $PASS passed, $FAIL failed"

if [[ $FAIL -gt 0 ]]; then
  echo "SMOKE TEST FAILED"
  exit 1
fi

echo "SMOKE TEST PASSED"
