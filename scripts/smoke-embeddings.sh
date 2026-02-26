#!/usr/bin/env bash
# smoke-embeddings.sh — Run embedding provider smoke tests.
#
# Usage:
#   ./scripts/smoke-embeddings.sh           # run all available tests
#   ./scripts/smoke-embeddings.sh local     # local ONNX only
#   ./scripts/smoke-embeddings.sh voyage    # Voyage API only
#   ./scripts/smoke-embeddings.sh all       # both (default)
#
# The Voyage test requires VOYAGE_API_KEY (env or .env file).
# The local test requires no credentials (downloads model on first run).

set -euo pipefail

SCRIPTS_DIR="$(cd "$(dirname "$0")" && pwd)"
PROVIDER="${1:-all}"

TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0

run_test() {
  local name="$1" script="$2"
  echo ""
  echo "================================================================"
  echo "  SMOKE TEST: $name"
  echo "================================================================"
  if "$script"; then
    echo ""
    echo "  >>> $name: PASSED"
    TOTAL_PASS=$((TOTAL_PASS + 1))
  else
    echo ""
    echo "  >>> $name: FAILED"
    TOTAL_FAIL=$((TOTAL_FAIL + 1))
  fi
}

skip_test() {
  local name="$1" reason="$2"
  echo ""
  echo "================================================================"
  echo "  SMOKE TEST: $name — SKIPPED ($reason)"
  echo "================================================================"
  TOTAL_SKIP=$((TOTAL_SKIP + 1))
}

# --------------------------------------------------------------------------
# Local ONNX provider
# --------------------------------------------------------------------------
if [[ "$PROVIDER" == "all" || "$PROVIDER" == "local" ]]; then
  run_test "Local ONNX (fastembed)" "$SCRIPTS_DIR/smoke-local.sh"
fi

# --------------------------------------------------------------------------
# Voyage API provider
# --------------------------------------------------------------------------
if [[ "$PROVIDER" == "all" || "$PROVIDER" == "voyage" ]]; then
  REPO_ROOT="$(cd "$SCRIPTS_DIR/.." && pwd)"
  # Check for API key before attempting
  VOYAGE_KEY="${VOYAGE_API_KEY:-}"
  if [[ -z "$VOYAGE_KEY" && -f "$REPO_ROOT/.env" ]]; then
    # Peek at .env without polluting our env (POSIX-compatible, no -P)
    VOYAGE_KEY=$(sed -n 's/^VOYAGE_API_KEY=//p' "$REPO_ROOT/.env" 2>/dev/null || true)
  fi

  if [[ -n "$VOYAGE_KEY" ]]; then
    run_test "Voyage API" "$SCRIPTS_DIR/smoke-voyage.sh"
  else
    skip_test "Voyage API" "VOYAGE_API_KEY not set"
  fi
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo "================================================================"
echo "  SUMMARY: $TOTAL_PASS passed, $TOTAL_FAIL failed, $TOTAL_SKIP skipped"
echo "================================================================"

if [[ $TOTAL_FAIL -gt 0 ]]; then
  exit 1
fi

echo "ALL SMOKE TESTS PASSED"
