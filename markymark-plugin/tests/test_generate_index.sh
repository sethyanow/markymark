#!/usr/bin/env bash
# Tests for generate-index.sh — docs_index generation from directory tree.
#
# Run: bash markymark-plugin/tests/test_generate_index.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="${SCRIPT_DIR}/.."
GENERATE_INDEX="${PLUGIN_DIR}/skills/export-docs-index/scripts/generate-index.sh"

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

# Create a temporary directory for test fixtures
setup_test_env() {
    TEST_DIR="$(mktemp -d)"
}

cleanup_test_env() {
    rm -rf "${TEST_DIR}"
}

# ─── Test: Script exists and is executable ───────────────────────
test_script_exists() {
    if [[ -x "${GENERATE_INDEX}" ]]; then
        pass "generate-index.sh exists and is executable"
    else
        fail "generate-index.sh exists and is executable" "executable file" "not found or not executable"
    fi
}

# ─── Test: Simple flat directory ─────────────────────────────────
# Directory with 3 .md files in root -> [name]|root: ./path|.:{a.md,b.md,c.md}
test_simple_flat_dir() {
    setup_test_env
    touch "${TEST_DIR}/alpha.md" "${TEST_DIR}/beta.md" "${TEST_DIR}/gamma.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "test-section" 2>/dev/null)" || {
        fail "simple flat dir" "[test-section]|root: ...|.:{alpha.md,beta.md,gamma.md}" "script failed"
        cleanup_test_env
        return
    }

    local expected="[test-section]|root: ${TEST_DIR}|.:{alpha.md,beta.md,gamma.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "simple flat dir"
    else
        fail "simple flat dir" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Nested subdirectories ─────────────────────────────────
# Directory with core/ and advanced/ subdirs
test_nested_subdirs() {
    setup_test_env
    mkdir -p "${TEST_DIR}/advanced" "${TEST_DIR}/core"
    touch "${TEST_DIR}/core/types.md" "${TEST_DIR}/core/traits.md"
    touch "${TEST_DIR}/advanced/async.md" "${TEST_DIR}/advanced/ffi.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "docs" 2>/dev/null)" || {
        fail "nested subdirs" "pipe-delimited with core and advanced" "script failed"
        cleanup_test_env
        return
    }

    local expected="[docs]|root: ${TEST_DIR}|advanced:{async.md,ffi.md}|core:{traits.md,types.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "nested subdirs"
    else
        fail "nested subdirs" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Mixed root and subdirs ────────────────────────────────
# Files in root + subdirectory -> both .:{root-files} and subdir:{files}
test_mixed_root_and_subdirs() {
    setup_test_env
    touch "${TEST_DIR}/README.md" "${TEST_DIR}/index.md"
    mkdir -p "${TEST_DIR}/guides"
    touch "${TEST_DIR}/guides/setup.md" "${TEST_DIR}/guides/usage.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "mixed" 2>/dev/null)" || {
        fail "mixed root and subdirs" "both .:{} and guides:{}" "script failed"
        cleanup_test_env
        return
    }

    local expected="[mixed]|root: ${TEST_DIR}|.:{README.md,index.md}|guides:{setup.md,usage.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "mixed root and subdirs"
    else
        fail "mixed root and subdirs" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Alphabetical sort ────────────────────────────────────
# Files and dirs output in alphabetical order
test_alphabetical_sort() {
    setup_test_env
    mkdir -p "${TEST_DIR}/zebra" "${TEST_DIR}/apple"
    touch "${TEST_DIR}/zebra/z.md" "${TEST_DIR}/zebra/a.md"
    touch "${TEST_DIR}/apple/m.md" "${TEST_DIR}/apple/b.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "sorted" 2>/dev/null)" || {
        fail "alphabetical sort" "apple before zebra, files sorted" "script failed"
        cleanup_test_env
        return
    }

    local expected="[sorted]|root: ${TEST_DIR}|apple:{b.md,m.md}|zebra:{a.md,z.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "alphabetical sort"
    else
        fail "alphabetical sort" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Hidden files excluded ─────────────────────────────────
test_hidden_files_excluded() {
    setup_test_env
    touch "${TEST_DIR}/visible.md" "${TEST_DIR}/.hidden.md"
    mkdir -p "${TEST_DIR}/.git"
    touch "${TEST_DIR}/.git/config.md"
    mkdir -p "${TEST_DIR}/.obsidian"
    touch "${TEST_DIR}/.obsidian/workspace.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "no-hidden" 2>/dev/null)" || {
        fail "hidden files excluded" "only visible.md" "script failed"
        cleanup_test_env
        return
    }

    local expected="[no-hidden]|root: ${TEST_DIR}|.:{visible.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "hidden files excluded"
    else
        fail "hidden files excluded" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Empty directory ───────────────────────────────────────
test_empty_directory() {
    setup_test_env
    # No .md files

    local output exit_code stderr_output
    stderr_output="$("${GENERATE_INDEX}" "${TEST_DIR}" "empty" 2>&1 1>/dev/null)" && exit_code=0 || exit_code=$?
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "empty" 2>/dev/null)" && true

    if [[ ${exit_code} -eq 0 ]] && [[ -z "${output}" ]]; then
        pass "empty directory (exit 0, empty output)"
    else
        fail "empty directory" "exit 0, empty stdout" "exit=${exit_code}, output='${output}'"
    fi

    cleanup_test_env
}

# ─── Test: Missing directory ─────────────────────────────────────
test_missing_directory() {
    local output exit_code
    output="$("${GENERATE_INDEX}" "/nonexistent/path" "missing" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]]; then
        pass "missing directory (exit ${exit_code})"
    else
        fail "missing directory" "non-zero exit" "exit=0, output='${output}'"
    fi

    cleanup_test_env 2>/dev/null || true
}

# ─── Test: Missing section name ──────────────────────────────────
test_missing_section_name() {
    setup_test_env
    local output exit_code
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]]; then
        pass "missing section name (exit ${exit_code})"
    else
        fail "missing section name" "non-zero exit" "exit=0, output='${output}'"
    fi

    cleanup_test_env
}

# ─── Test: Instruction text ──────────────────────────────────────
test_instruction_text() {
    setup_test_env
    touch "${TEST_DIR}/doc.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "instruct" "Read these docs first." 2>/dev/null)" || {
        fail "instruction text" "instruction after root" "script failed"
        cleanup_test_env
        return
    }

    local expected="[instruct]|root: ${TEST_DIR}|Read these docs first.|.:{doc.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "instruction text"
    else
        fail "instruction text" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Deeply nested directories ────────────────────────────
# 3+ levels deep correctly groups by immediate subdirectory relative to root
test_deeply_nested() {
    setup_test_env
    mkdir -p "${TEST_DIR}/level1/level2/level3"
    touch "${TEST_DIR}/level1/top.md"
    touch "${TEST_DIR}/level1/level2/mid.md"
    touch "${TEST_DIR}/level1/level2/level3/deep.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "deep" 2>/dev/null)" || {
        fail "deeply nested" "groups by immediate subdir" "script failed"
        cleanup_test_env
        return
    }

    # Files should be grouped by their path relative to root
    # level1/top.md -> level1:{top.md}
    # level1/level2/mid.md -> level1/level2:{mid.md}  OR  level1:{level2/mid.md}
    # The convention from CLAUDE.md shows subdirectory grouping uses immediate subdirs
    # e.g., zig:{AGENTS.md,...,00-general/installation.md,...}
    # So files in nested dirs use relative paths from the immediate subdir
    local expected="[deep]|level1:{top.md,level2/mid.md,level2/level3/deep.md}"
    if [[ "${output}" == *"level1:"* ]]; then
        # Check it groups under level1 with nested paths
        if [[ "${output}" == *"level2/mid.md"* ]] && [[ "${output}" == *"level2/level3/deep.md"* ]]; then
            pass "deeply nested (files grouped with nested paths)"
        else
            fail "deeply nested" "nested paths like level2/mid.md" "${output}"
        fi
    else
        fail "deeply nested" "grouping under level1:" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Special chars in filename ─────────────────────────────
test_special_chars_in_filename() {
    setup_test_env
    touch "${TEST_DIR}/normal.md"
    touch "${TEST_DIR}/my file (1).md"
    touch "${TEST_DIR}/über-docs.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "special" 2>/dev/null)" || {
        fail "special chars in filename" "all files listed" "script failed"
        cleanup_test_env
        return
    }

    if [[ "${output}" == *"my file (1).md"* ]] && [[ "${output}" == *"über-docs.md"* ]] && [[ "${output}" == *"normal.md"* ]]; then
        pass "special chars in filename"
    else
        fail "special chars in filename" "all 3 files including special chars" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Non-md files ignored ──────────────────────────────────
test_non_md_files_ignored() {
    setup_test_env
    touch "${TEST_DIR}/doc.md" "${TEST_DIR}/readme.txt" "${TEST_DIR}/script.sh" "${TEST_DIR}/data.json"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "mdonly" 2>/dev/null)" || {
        fail "non-md files ignored" "only doc.md" "script failed"
        cleanup_test_env
        return
    }

    local expected="[mdonly]|root: ${TEST_DIR}|.:{doc.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "non-md files ignored"
    else
        fail "non-md files ignored" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: node_modules and .git excluded ────────────────────────
test_ignored_directories() {
    setup_test_env
    touch "${TEST_DIR}/real.md"
    mkdir -p "${TEST_DIR}/node_modules/pkg"
    touch "${TEST_DIR}/node_modules/pkg/readme.md"
    mkdir -p "${TEST_DIR}/.git/objects"
    touch "${TEST_DIR}/.git/objects/note.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "clean" 2>/dev/null)" || {
        fail "ignored directories" "only real.md" "script failed"
        cleanup_test_env
        return
    }

    local expected="[clean]|root: ${TEST_DIR}|.:{real.md}"
    if [[ "${output}" == "${expected}" ]]; then
        pass "ignored directories (node_modules, .git excluded)"
    else
        fail "ignored directories" "${expected}" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: No arguments ─────────────────────────────────────────
test_no_arguments() {
    local output exit_code
    output="$("${GENERATE_INDEX}" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]]; then
        pass "no arguments (exit ${exit_code})"
    else
        fail "no arguments" "non-zero exit" "exit=0"
    fi
}

# ─── Test: Output has no XML wrapper ─────────────────────────────
test_no_xml_wrapper() {
    setup_test_env
    touch "${TEST_DIR}/file.md"

    local output
    output="$("${GENERATE_INDEX}" "${TEST_DIR}" "nowrap" 2>/dev/null)" || {
        fail "no XML wrapper" "raw pipe-delimited" "script failed"
        cleanup_test_env
        return
    }

    if [[ "${output}" != *"<docs_index>"* ]] && [[ "${output}" != *"</docs_index>"* ]]; then
        pass "no XML wrapper in output"
    else
        fail "no XML wrapper" "no <docs_index> tags" "${output}"
    fi

    cleanup_test_env
}

# ─── Run all tests ───────────────────────────────────────────────
echo "=== export-docs-index generate-index.sh tests ==="
echo ""

test_script_exists
test_no_arguments
test_missing_section_name
test_missing_directory
test_simple_flat_dir
test_nested_subdirs
test_mixed_root_and_subdirs
test_alphabetical_sort
test_hidden_files_excluded
test_empty_directory
test_non_md_files_ignored
test_ignored_directories
test_instruction_text
test_deeply_nested
test_special_chars_in_filename
test_no_xml_wrapper

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
