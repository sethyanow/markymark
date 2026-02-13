#!/usr/bin/env bash
# Tests for select-binary.sh platform detection and binary selection.
#
# Run: bash markymark-plugin/tests/test_select_binary.sh
#
# Tests use a mock binary approach: create fake binaries in a temp bin/
# directory and verify that select-binary.sh picks the correct one.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="${SCRIPT_DIR}/.."
SELECT_BINARY="${PLUGIN_DIR}/scripts/select-binary.sh"

PASS=0
FAIL=0
TESTS_RUN=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

pass() {
    PASS=$((PASS + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
    printf "${GREEN}PASS${NC}: %s\n" "$1"
}

fail() {
    FAIL=$((FAIL + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
    printf "${RED}FAIL${NC}: %s\n  Expected: %s\n  Got:      %s\n" "$1" "$2" "$3"
}

# Create a temporary plugin directory structure for testing
setup_test_env() {
    TEST_DIR="$(mktemp -d)"
    mkdir -p "${TEST_DIR}/bin"
    mkdir -p "${TEST_DIR}/scripts"
    cp "${SELECT_BINARY}" "${TEST_DIR}/scripts/select-binary.sh"
    chmod +x "${TEST_DIR}/scripts/select-binary.sh"
}

cleanup_test_env() {
    rm -rf "${TEST_DIR}"
}

# ─── Test: Script exists and is executable ───────────────────────
test_script_exists() {
    if [[ -x "${SELECT_BINARY}" ]]; then
        pass "select-binary.sh exists and is executable"
    else
        fail "select-binary.sh exists and is executable" "executable file" "not found or not executable"
    fi
}

# ─── Test: Detects current platform correctly ────────────────────
test_detects_current_platform() {
    local os arch expected_target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) expected_target="aarch64-apple-darwin" ;;
                x86_64)        expected_target="x86_64-apple-darwin" ;;
                *)             fail "detects current platform" "known arch" "${arch}"; return ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) expected_target="aarch64-unknown-linux-gnu" ;;
                x86_64)        expected_target="x86_64-unknown-linux-gnu" ;;
                *)             fail "detects current platform" "known arch" "${arch}"; return ;;
            esac
            ;;
        *)
            # Can't test on this platform
            pass "detects current platform (skipped: unsupported test platform ${os})"
            return
            ;;
    esac

    setup_test_env

    # Create a mock binary that just prints its name
    local mock_binary="${TEST_DIR}/bin/markymark-${expected_target}"
    printf '#!/usr/bin/env bash\necho "SELECTED: %s"\n' "${expected_target}" > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == "SELECTED: ${expected_target}" ]]; then
        pass "detects current platform → ${expected_target}"
    else
        fail "detects current platform → ${expected_target}" "SELECTED: ${expected_target}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Forwards arguments to binary ──────────────────────────
test_forwards_arguments() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) target="aarch64-apple-darwin" ;;
                x86_64)        target="x86_64-apple-darwin" ;;
                *)             pass "forwards arguments (skipped)"; return ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
                x86_64)        target="x86_64-unknown-linux-gnu" ;;
                *)             pass "forwards arguments (skipped)"; return ;;
            esac
            ;;
        *)
            pass "forwards arguments (skipped: unsupported test platform ${os})"
            return
            ;;
    esac

    setup_test_env

    # Create mock binary that echoes all arguments
    local mock_binary="${TEST_DIR}/bin/markymark-${target}"
    printf '#!/usr/bin/env bash\necho "ARGS: $*"\n' > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" --lsp --foo bar 2>&1)" || true

    if [[ "${output}" == "ARGS: --lsp --foo bar" ]]; then
        pass "forwards arguments to selected binary"
    else
        fail "forwards arguments to selected binary" "ARGS: --lsp --foo bar" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Fails gracefully when binary is missing ───────────────
test_missing_binary() {
    setup_test_env

    # Don't create any mock binary — bin/ is empty
    local output exit_code
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]] && [[ "${output}" == *"binary not found"* ]]; then
        pass "fails gracefully when binary is missing (exit ${exit_code})"
    else
        fail "fails gracefully when binary is missing" "non-zero exit + 'binary not found'" "exit=${exit_code}, output=${output}"
    fi

    cleanup_test_env
}

# ─── Test: Error message includes hint ───────────────────────────
test_error_includes_hint() {
    setup_test_env

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == *"GitHub Releases"* ]]; then
        pass "error message includes GitHub Releases hint"
    else
        fail "error message includes GitHub Releases hint" "*GitHub Releases*" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Makes binary executable if needed ─────────────────────
test_makes_binary_executable() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) target="aarch64-apple-darwin" ;;
                x86_64)        target="x86_64-apple-darwin" ;;
                *)             pass "makes binary executable (skipped)"; return ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
                x86_64)        target="x86_64-unknown-linux-gnu" ;;
                *)             pass "makes binary executable (skipped)"; return ;;
            esac
            ;;
        *)
            pass "makes binary executable (skipped: unsupported test platform ${os})"
            return
            ;;
    esac

    setup_test_env

    # Create mock binary WITHOUT execute permission
    local mock_binary="${TEST_DIR}/bin/markymark-${target}"
    printf '#!/usr/bin/env bash\necho "EXECUTED"\n' > "${mock_binary}"
    chmod -x "${mock_binary}"

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == "EXECUTED" ]]; then
        pass "makes non-executable binary executable and runs it"
    else
        fail "makes non-executable binary executable and runs it" "EXECUTED" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Plugin directory structure is correct ─────────────────
test_plugin_structure() {
    local all_ok=true

    if [[ ! -f "${PLUGIN_DIR}/.claude-plugin/plugin.json" ]]; then
        fail "plugin.json exists" "file present" "missing"
        all_ok=false
    fi

    if [[ ! -f "${PLUGIN_DIR}/.lsp.json" ]]; then
        fail ".lsp.json exists" "file present" "missing"
        all_ok=false
    fi

    if [[ ! -f "${PLUGIN_DIR}/.mcp.json" ]]; then
        fail ".mcp.json exists" "file present" "missing"
        all_ok=false
    fi

    if [[ ! -d "${PLUGIN_DIR}/bin" ]]; then
        fail "bin/ directory exists" "directory present" "missing"
        all_ok=false
    fi

    if [[ ! -d "${PLUGIN_DIR}/scripts" ]]; then
        fail "scripts/ directory exists" "directory present" "missing"
        all_ok=false
    fi

    if ${all_ok}; then
        pass "plugin directory structure is complete"
    fi
}

# ─── Test: plugin.json is valid JSON ─────────────────────────────
test_plugin_json_valid() {
    if command -v python3 &>/dev/null; then
        if python3 -c "import json; json.load(open('${PLUGIN_DIR}/.claude-plugin/plugin.json'))" 2>/dev/null; then
            pass "plugin.json is valid JSON"
        else
            fail "plugin.json is valid JSON" "valid JSON" "parse error"
        fi
    elif command -v jq &>/dev/null; then
        if jq . "${PLUGIN_DIR}/.claude-plugin/plugin.json" >/dev/null 2>&1; then
            pass "plugin.json is valid JSON"
        else
            fail "plugin.json is valid JSON" "valid JSON" "parse error"
        fi
    else
        pass "plugin.json is valid JSON (skipped: no json parser available)"
    fi
}

# ─── Test: .lsp.json references CLAUDE_PLUGIN_ROOT ───────────────
test_lsp_json_uses_plugin_root() {
    if command -v python3 &>/dev/null; then
        local cmd
        cmd=$(python3 -c "import json; print(json.load(open('${PLUGIN_DIR}/.lsp.json'))['markdown']['command'])" 2>/dev/null)
        if [[ "${cmd}" == *'${CLAUDE_PLUGIN_ROOT}'* ]]; then
            pass ".lsp.json uses \${CLAUDE_PLUGIN_ROOT} in command"
        else
            fail ".lsp.json uses \${CLAUDE_PLUGIN_ROOT} in command" '*\${CLAUDE_PLUGIN_ROOT}*' "${cmd}"
        fi
    else
        pass ".lsp.json uses \${CLAUDE_PLUGIN_ROOT} (skipped: no python3)"
    fi
}

# ─── Test: .mcp.json references CLAUDE_PLUGIN_ROOT ───────────────
test_mcp_json_uses_plugin_root() {
    if command -v python3 &>/dev/null; then
        local cmd
        cmd=$(python3 -c "import json; print(json.load(open('${PLUGIN_DIR}/.mcp.json'))['mcpServers']['markymark']['command'])" 2>/dev/null)
        if [[ "${cmd}" == *'${CLAUDE_PLUGIN_ROOT}'* ]]; then
            pass ".mcp.json uses \${CLAUDE_PLUGIN_ROOT} in command"
        else
            fail ".mcp.json uses \${CLAUDE_PLUGIN_ROOT} in command" '*\${CLAUDE_PLUGIN_ROOT}*' "${cmd}"
        fi
    else
        pass ".mcp.json uses \${CLAUDE_PLUGIN_ROOT} (skipped: no python3)"
    fi
}

# ─── Run all tests ───────────────────────────────────────────────
echo "=== markymark-plugin tests ==="
echo ""

test_script_exists
test_plugin_structure
test_plugin_json_valid
test_lsp_json_uses_plugin_root
test_mcp_json_uses_plugin_root
test_detects_current_platform
test_forwards_arguments
test_missing_binary
test_error_includes_hint
test_makes_binary_executable

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
