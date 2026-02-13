#!/usr/bin/env bash
# Tests for markymark plugin hooks integration.
#
# Run: bash markymark-plugin/tests/test_hooks.sh
#
# Validates that hooks.json and suggest-lsp.sh are properly integrated
# into the plugin directory (not just in examples/).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="${SCRIPT_DIR}/.."
HOOKS_DIR="${PLUGIN_DIR}/hooks"

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

# ─── Test: hooks directory exists ──────────────────────────────────
test_hooks_dir_exists() {
    if [[ -d "${HOOKS_DIR}" ]]; then
        pass "hooks/ directory exists"
    else
        fail "hooks/ directory exists" "directory present" "missing"
    fi
}

# ─── Test: hooks.json exists and is valid JSON ─────────────────────
test_hooks_json_valid() {
    local hooks_json="${HOOKS_DIR}/hooks.json"
    if [[ ! -f "${hooks_json}" ]]; then
        fail "hooks.json is valid JSON" "file present" "missing"
        return
    fi

    if jq . "${hooks_json}" >/dev/null 2>&1; then
        pass "hooks.json is valid JSON"
    else
        fail "hooks.json is valid JSON" "valid JSON" "parse error"
    fi
}

# ─── Test: hooks.json has correct structure ────────────────────────
test_hooks_json_structure() {
    local hooks_json="${HOOKS_DIR}/hooks.json"
    if [[ ! -f "${hooks_json}" ]]; then
        fail "hooks.json has correct structure" "file present" "missing"
        return
    fi

    local has_description has_hooks has_pre_tool_use
    has_description=$(jq -r 'has("description")' "${hooks_json}" 2>/dev/null)
    has_hooks=$(jq -r 'has("hooks")' "${hooks_json}" 2>/dev/null)
    has_pre_tool_use=$(jq -r '.hooks | has("PreToolUse")' "${hooks_json}" 2>/dev/null)

    if [[ "${has_description}" == "true" ]] && \
       [[ "${has_hooks}" == "true" ]] && \
       [[ "${has_pre_tool_use}" == "true" ]]; then
        pass "hooks.json has description + hooks.PreToolUse structure"
    else
        fail "hooks.json has description + hooks.PreToolUse structure" \
            "description=true, hooks=true, PreToolUse=true" \
            "description=${has_description}, hooks=${has_hooks}, PreToolUse=${has_pre_tool_use}"
    fi
}

# ─── Test: PreToolUse hook matches Read tool ───────────────────────
test_pre_tool_use_matcher() {
    local hooks_json="${HOOKS_DIR}/hooks.json"
    if [[ ! -f "${hooks_json}" ]]; then
        fail "PreToolUse matches Read tool" "file present" "missing"
        return
    fi

    local matcher
    matcher=$(jq -r '.hooks.PreToolUse[0].matcher' "${hooks_json}" 2>/dev/null)

    if [[ "${matcher}" == "Read" ]]; then
        pass "PreToolUse hook matches Read tool"
    else
        fail "PreToolUse hook matches Read tool" "Read" "${matcher}"
    fi
}

# ─── Test: suggest-lsp.sh exists and is executable ─────────────────
test_suggest_lsp_exists() {
    local script="${HOOKS_DIR}/suggest-lsp.sh"
    if [[ -x "${script}" ]]; then
        pass "suggest-lsp.sh exists and is executable"
    else
        if [[ -f "${script}" ]]; then
            fail "suggest-lsp.sh exists and is executable" "executable" "exists but not executable"
        else
            fail "suggest-lsp.sh exists and is executable" "executable file" "missing"
        fi
    fi
}

# ─── Test: suggest-lsp.sh returns valid JSON for .md input ─────────
test_suggest_lsp_md_input() {
    local script="${HOOKS_DIR}/suggest-lsp.sh"
    if [[ ! -x "${script}" ]]; then
        fail "suggest-lsp.sh returns valid JSON for .md input" "executable" "not found"
        return
    fi

    local input='{"tool_name":"Read","tool_input":{"file_path":"test.md"}}'
    local output
    output=$(echo "${input}" | bash "${script}" 2>/dev/null)

    if echo "${output}" | jq . >/dev/null 2>&1; then
        # Also check it has the expected fields
        local has_decision has_message
        has_decision=$(echo "${output}" | jq -r '.hookSpecificOutput.permissionDecision' 2>/dev/null)
        has_message=$(echo "${output}" | jq -r 'has("systemMessage")' 2>/dev/null)

        if [[ "${has_decision}" == "allow" ]] && [[ "${has_message}" == "true" ]]; then
            pass "suggest-lsp.sh returns valid JSON with allow + systemMessage for .md"
        else
            fail "suggest-lsp.sh returns valid JSON with allow + systemMessage for .md" \
                "decision=allow, systemMessage=true" \
                "decision=${has_decision}, systemMessage=${has_message}"
        fi
    else
        fail "suggest-lsp.sh returns valid JSON for .md input" "valid JSON" "${output}"
    fi
}

# ─── Test: suggest-lsp.sh returns valid JSON for .mdx input ────────
test_suggest_lsp_mdx_input() {
    local script="${HOOKS_DIR}/suggest-lsp.sh"
    if [[ ! -x "${script}" ]]; then
        fail "suggest-lsp.sh returns valid JSON for .mdx input" "executable" "not found"
        return
    fi

    local input='{"tool_name":"Read","tool_input":{"file_path":"test.mdx"}}'
    local output
    output=$(echo "${input}" | bash "${script}" 2>/dev/null)

    if echo "${output}" | jq -r '.hookSpecificOutput.permissionDecision' 2>/dev/null | grep -q "allow"; then
        pass "suggest-lsp.sh returns allow for .mdx files"
    else
        fail "suggest-lsp.sh returns allow for .mdx files" "allow" "${output}"
    fi
}

# ─── Test: suggest-lsp.sh returns empty for non-.md input ──────────
test_suggest_lsp_non_md() {
    local script="${HOOKS_DIR}/suggest-lsp.sh"
    if [[ ! -x "${script}" ]]; then
        fail "suggest-lsp.sh returns empty for non-.md input" "executable" "not found"
        return
    fi

    local input='{"tool_name":"Read","tool_input":{"file_path":"test.rs"}}'
    local output
    output=$(echo "${input}" | bash "${script}" 2>/dev/null)

    if [[ -z "${output}" ]]; then
        pass "suggest-lsp.sh returns empty for non-.md files"
    else
        fail "suggest-lsp.sh returns empty for non-.md files" "empty output" "${output}"
    fi
}

# ─── Test: hooks.json uses ${CLAUDE_PLUGIN_ROOT} in command ────────
test_uses_plugin_root() {
    local hooks_json="${HOOKS_DIR}/hooks.json"
    if [[ ! -f "${hooks_json}" ]]; then
        fail "hooks.json uses \${CLAUDE_PLUGIN_ROOT}" "file present" "missing"
        return
    fi

    local command
    command=$(jq -r '.hooks.PreToolUse[0].hooks[0].command' "${hooks_json}" 2>/dev/null)

    if [[ "${command}" == *'${CLAUDE_PLUGIN_ROOT}'* ]]; then
        pass "hooks.json uses \${CLAUDE_PLUGIN_ROOT} in command path"
    else
        fail "hooks.json uses \${CLAUDE_PLUGIN_ROOT} in command path" \
            '*\${CLAUDE_PLUGIN_ROOT}*' "${command}"
    fi
}

# ─── Run all tests ─────────────────────────────────────────────────
echo "=== markymark-plugin hook tests ==="
echo ""

test_hooks_dir_exists
test_hooks_json_valid
test_hooks_json_structure
test_pre_tool_use_matcher
test_suggest_lsp_exists
test_suggest_lsp_md_input
test_suggest_lsp_mdx_input
test_suggest_lsp_non_md
test_uses_plugin_root

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
