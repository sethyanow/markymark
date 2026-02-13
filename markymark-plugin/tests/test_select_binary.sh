#!/usr/bin/env bash
# Tests for select-binary.sh — bundled binary model.
#
# Run: bash markymark-plugin/tests/test_select_binary.sh
#
# In the bundled model, CI pre-packages per-platform plugin archives.
# Each archive contains a single bin/markymark binary (already correct
# for the platform). select-binary.sh just finds and runs it — no
# platform-target naming, no download, no multi-binary detection.

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

# ─── Test: Finds and executes bin/markymark (bundled binary) ─────
# In the new model, each platform archive contains a single
# bin/markymark binary. The script should look for bin/markymark
# (not bin/markymark-{target}).
test_bundled_binary() {
    setup_test_env

    # Create mock binary at bin/markymark (the bundled name)
    local mock_binary="${TEST_DIR}/bin/markymark"
    printf '#!/usr/bin/env bash\necho "BUNDLED"\n' > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == "BUNDLED" ]]; then
        pass "finds and executes bundled bin/markymark"
    else
        fail "finds and executes bundled bin/markymark" "BUNDLED" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Forwards arguments to bundled binary ──────────────────
test_forwards_arguments() {
    setup_test_env

    local mock_binary="${TEST_DIR}/bin/markymark"
    printf '#!/usr/bin/env bash\necho "ARGS: $*"\n' > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" --lsp --foo bar 2>&1)" || true

    if [[ "${output}" == "ARGS: --lsp --foo bar" ]]; then
        pass "forwards arguments to bundled binary"
    else
        fail "forwards arguments to bundled binary" "ARGS: --lsp --foo bar" "${output}"
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

# ─── Test: Error suggests platform-specific archive download ─────
# When the bundled binary is missing, the error should point the user
# to the correct platform-specific archive on GitHub Releases.
test_error_suggests_platform_archive() {
    setup_test_env

    local output
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == *"GitHub Releases"* ]]; then
        pass "error message mentions GitHub Releases"
    else
        fail "error message mentions GitHub Releases" "*GitHub Releases*" "${output}"
    fi

    # Should also mention the platform-specific archive name
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    local expected_target=""

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) expected_target="aarch64-apple-darwin" ;;
                x86_64)        expected_target="x86_64-apple-darwin" ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) expected_target="aarch64-unknown-linux-gnu" ;;
                x86_64)        expected_target="x86_64-unknown-linux-gnu" ;;
            esac
            ;;
    esac

    if [[ -n "${expected_target}" ]]; then
        if [[ "${output}" == *"${expected_target}"* ]]; then
            pass "error message includes platform target (${expected_target})"
        else
            fail "error message includes platform target" "*${expected_target}*" "${output}"
        fi
    else
        pass "error message includes platform target (skipped: unknown platform)"
    fi

    cleanup_test_env
}

# ─── Test: Makes binary executable if needed ─────────────────────
test_makes_binary_executable() {
    setup_test_env

    # Create mock binary WITHOUT execute permission
    local mock_binary="${TEST_DIR}/bin/markymark"
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

# ─── Test: Does NOT look for platform-specific binary names ──────
# The old model used bin/markymark-{target}. The new model should
# NOT find those — only bin/markymark.
test_ignores_platform_specific_binaries() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) target="aarch64-apple-darwin" ;;
                x86_64)        target="x86_64-apple-darwin" ;;
                *)             pass "ignores platform binaries (skipped)"; return ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
                x86_64)        target="x86_64-unknown-linux-gnu" ;;
                *)             pass "ignores platform binaries (skipped)"; return ;;
            esac
            ;;
        *)
            pass "ignores platform binaries (skipped: unsupported ${os})"
            return
            ;;
    esac

    setup_test_env

    # Create ONLY a platform-specific binary (old model)
    local old_binary="${TEST_DIR}/bin/markymark-${target}"
    printf '#!/usr/bin/env bash\necho "OLD MODEL"\n' > "${old_binary}"
    chmod +x "${old_binary}"

    # Do NOT create bin/markymark — only the old-style name exists

    local output exit_code
    output="$("${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]]; then
        pass "ignores platform-specific binary (old model not found, exit ${exit_code})"
    else
        fail "ignores platform-specific binary" "non-zero exit (binary not found)" "exit=0, output=${output}"
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
echo "=== markymark-plugin tests (bundled binary model) ==="
echo ""

test_script_exists
test_plugin_structure
test_plugin_json_valid
test_lsp_json_uses_plugin_root
test_mcp_json_uses_plugin_root
test_bundled_binary
test_forwards_arguments
test_missing_binary
test_error_suggests_platform_archive
test_makes_binary_executable
test_ignores_platform_specific_binaries

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
