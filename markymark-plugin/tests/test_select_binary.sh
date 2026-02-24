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

# ─── Test: Missing binary attempts download ──────────────────────
# When binary is missing, script should attempt to download from
# GitHub Releases before giving up. We mock curl to test this.
test_missing_binary_attempts_download() {
    setup_test_env

    # Create a mock curl that simulates a download failure
    mkdir -p "${TEST_DIR}/mock-bin"
    printf '#!/usr/bin/env bash\necho "mock-curl: $*" >&2\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output exit_code
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

    # Should mention attempting download
    if [[ "${output}" == *"downloading"* ]] || [[ "${output}" == *"Downloading"* ]]; then
        pass "missing binary attempts download"
    else
        fail "missing binary attempts download" "*downloading*" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Download failure falls back to manual instructions ─────
test_download_failure_shows_manual_instructions() {
    setup_test_env

    # Create a mock curl that fails
    mkdir -p "${TEST_DIR}/mock-bin"
    printf '#!/usr/bin/env bash\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output exit_code
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -ne 0 ]] && [[ "${output}" == *"GitHub Releases"* ]]; then
        pass "download failure shows manual instructions (exit ${exit_code})"
    else
        fail "download failure shows manual instructions" "non-zero exit + 'GitHub Releases'" "exit=${exit_code}, output=${output}"
    fi

    cleanup_test_env
}

# ─── Test: Successful download places binary and executes ─────────
test_successful_download_executes() {
    setup_test_env

    # Create a mock curl that "downloads" a fake binary
    mkdir -p "${TEST_DIR}/mock-bin"
    cat > "${TEST_DIR}/mock-bin/curl" << 'MOCK_CURL'
#!/usr/bin/env bash
# Simulate successful download by writing a mock binary to the -o path
for i in $(seq 1 $#); do
    if [[ "${!i}" == "-o" ]]; then
        next=$((i + 1))
        outfile="${!next}"
        printf '#!/usr/bin/env bash\necho "DOWNLOADED"\n' > "${outfile}"
        chmod +x "${outfile}"
        exit 0
    fi
done
exit 1
MOCK_CURL
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output exit_code
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ "${output}" == *"DOWNLOADED"* ]]; then
        pass "successful download places binary and executes it"
    else
        fail "successful download places binary and executes it" "*DOWNLOADED*" "exit=${exit_code}, output=${output}"
    fi

    cleanup_test_env
}

# ─── Test: Download URL contains correct platform target ──────────
test_download_url_has_correct_target() {
    setup_test_env

    # Create a mock curl that logs the URL it receives
    mkdir -p "${TEST_DIR}/mock-bin"
    printf '#!/usr/bin/env bash\necho "CURL_ARGS: $*" >&2\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    local os arch expected_target
    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) expected_target="aarch64-apple-darwin" ;;
                x86_64)        expected_target="x86_64-apple-darwin" ;;
                *)             pass "download URL target (skipped: unknown arch)"; cleanup_test_env; return ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) expected_target="aarch64-unknown-linux-gnu" ;;
                x86_64)        expected_target="x86_64-unknown-linux-gnu" ;;
                *)             pass "download URL target (skipped: unknown arch)"; cleanup_test_env; return ;;
            esac
            ;;
        *)
            pass "download URL target (skipped: unsupported ${os})"
            cleanup_test_env
            return
            ;;
    esac

    if [[ "${output}" == *"${expected_target}"* ]]; then
        pass "download URL contains correct platform target (${expected_target})"
    else
        fail "download URL contains correct platform target" "*${expected_target}*" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Error suggests platform-specific archive download ─────
# When download fails, the error should point the user to the correct
# platform-specific archive on GitHub Releases.
test_error_suggests_platform_archive() {
    setup_test_env

    # Create a mock curl that fails (so we see the fallback error)
    mkdir -p "${TEST_DIR}/mock-bin"
    printf '#!/usr/bin/env bash\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

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

    # Mock curl to fail so download fallback doesn't mask the test
    mkdir -p "${TEST_DIR}/mock-bin"
    printf '#!/usr/bin/env bash\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    local output exit_code
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" && exit_code=0 || exit_code=$?

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

# ─── Test: marketplace.json exists and is valid ───────────────────
REPO_ROOT="${PLUGIN_DIR}/.."
test_marketplace_json_valid() {
    local mktplace="${REPO_ROOT}/.claude-plugin/marketplace.json"
    if [[ ! -f "${mktplace}" ]]; then
        fail "marketplace.json exists" "file present" "missing at ${mktplace}"
        return
    fi

    if command -v python3 &>/dev/null; then
        if python3 -c "import json; json.load(open('${mktplace}'))" 2>/dev/null; then
            pass "marketplace.json is valid JSON"
        else
            fail "marketplace.json is valid JSON" "valid JSON" "parse error"
            return
        fi
    elif command -v jq &>/dev/null; then
        if jq . "${mktplace}" >/dev/null 2>&1; then
            pass "marketplace.json is valid JSON"
        else
            fail "marketplace.json is valid JSON" "valid JSON" "parse error"
            return
        fi
    else
        pass "marketplace.json is valid JSON (skipped: no json parser available)"
        return
    fi
}

# ─── Test: marketplace.json has required fields ───────────────────
test_marketplace_json_fields() {
    local mktplace="${REPO_ROOT}/.claude-plugin/marketplace.json"
    if [[ ! -f "${mktplace}" ]]; then
        fail "marketplace.json fields" "file present" "missing"
        return
    fi

    if ! command -v python3 &>/dev/null; then
        pass "marketplace.json fields (skipped: no python3)"
        return
    fi

    local result
    result=$(python3 -c "
import json, sys
m = json.load(open('${mktplace}'))
errors = []
if 'name' not in m: errors.append('missing name')
if 'owner' not in m: errors.append('missing owner')
if 'plugins' not in m: errors.append('missing plugins')
elif not m['plugins']: errors.append('plugins array is empty')
else:
    p = m['plugins'][0]
    if 'name' not in p: errors.append('plugin missing name')
    if 'source' not in p: errors.append('plugin missing source')
if errors:
    print('ERRORS: ' + ', '.join(errors))
    sys.exit(1)
else:
    print('OK')
" 2>&1) || true

    if [[ "${result}" == "OK" ]]; then
        pass "marketplace.json has required fields (name, owner, plugins with source)"
    else
        fail "marketplace.json has required fields" "OK" "${result}"
    fi
}

# ─── Test: marketplace.json plugin source path exists ─────────────
test_marketplace_plugin_source_exists() {
    local mktplace="${REPO_ROOT}/.claude-plugin/marketplace.json"
    if [[ ! -f "${mktplace}" ]]; then
        fail "marketplace plugin source" "file present" "marketplace.json missing"
        return
    fi

    if ! command -v python3 &>/dev/null; then
        pass "marketplace plugin source exists (skipped: no python3)"
        return
    fi

    local source_path
    source_path=$(python3 -c "import json; print(json.load(open('${mktplace}'))['plugins'][0]['source'])" 2>/dev/null)

    if [[ -z "${source_path}" ]]; then
        fail "marketplace plugin source path exists" "plugins[0].source present" "missing or invalid in ${mktplace}"
        return
    fi

    # Resolve source path relative to repo root (marketplace.json convention).
    # Absolute paths are used as-is; relative paths resolve from REPO_ROOT.
    local resolved
    if [[ "${source_path}" = /* ]]; then
        resolved="${source_path}"
    else
        resolved="${REPO_ROOT}/${source_path#./}"
    fi
    if [[ -d "${resolved}" ]]; then
        pass "marketplace plugin source path exists (${source_path})"
    else
        fail "marketplace plugin source path exists" "${source_path} is a directory" "not found at ${resolved}"
    fi
}

# ─── Helper: create mock uname for Windows simulation ─────────────
create_mock_uname() {
    local mock_dir="$1"
    local mock_os="${2:-MINGW64_NT-10.0-19045}"
    local mock_arch="${3:-x86_64}"
    cat > "${mock_dir}/uname" << MOCK_UNAME
#!/usr/bin/env bash
for arg in "\$@"; do
    case "\${arg}" in
        -s) echo "${mock_os}"; exit 0 ;;
        -m) echo "${mock_arch}"; exit 0 ;;
    esac
done
# No flags — uname with no args prints kernel name
echo "${mock_os}"
MOCK_UNAME
    chmod +x "${mock_dir}/uname"
}

# ─── Test: Windows binary path gets .exe suffix ───────────────────
# Regression test for marky-vxgg: On Windows, bin/markymark should be
# bin/markymark.exe. Without .exe, the bundled check fails and the
# download constructs an incorrect URL.
test_windows_binary_has_exe_suffix() {
    setup_test_env

    # Create mock uname that reports Windows (MINGW64) and mock curl to
    # prevent real network requests if binary path resolution fails
    mkdir -p "${TEST_DIR}/mock-bin"
    create_mock_uname "${TEST_DIR}/mock-bin" "MINGW64_NT-10.0-19045" "x86_64"
    printf '#!/usr/bin/env bash\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    # Place a binary at bin/markymark.exe (what CI produces)
    local mock_binary="${TEST_DIR}/bin/markymark.exe"
    printf '#!/usr/bin/env bash\necho "WINDOWS_EXE"\n' > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == "WINDOWS_EXE" ]]; then
        pass "Windows binary path uses .exe suffix"
    else
        fail "Windows binary path uses .exe suffix" "WINDOWS_EXE" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Windows download URL has .exe suffix ───────────────────
# Regression test for marky-vxgg: Download URL must end in .exe for
# Windows targets, otherwise GitHub release asset 404s.
test_windows_download_url_has_exe_suffix() {
    setup_test_env

    # Mock uname as Windows
    mkdir -p "${TEST_DIR}/mock-bin"
    create_mock_uname "${TEST_DIR}/mock-bin" "MINGW64_NT-10.0-19045" "x86_64"

    # Mock curl that logs URL and fails
    printf '#!/usr/bin/env bash\necho "CURL_URL: $*" >&2\nexit 1\n' > "${TEST_DIR}/mock-bin/curl"
    chmod +x "${TEST_DIR}/mock-bin/curl"

    # Do NOT create bin/markymark.exe so download path triggers
    local output
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == *"markymark-x86_64-pc-windows-msvc.exe"* ]]; then
        pass "Windows download URL includes .exe suffix"
    else
        fail "Windows download URL includes .exe suffix" \
            "*markymark-x86_64-pc-windows-msvc.exe*" "${output}"
    fi

    cleanup_test_env
}

# ─── Test: Non-Windows binary path has no .exe suffix ─────────────
# Ensures the .exe fix doesn't break macOS/Linux paths.
test_non_windows_binary_no_exe_suffix() {
    setup_test_env

    # Mock uname as Linux
    mkdir -p "${TEST_DIR}/mock-bin"
    create_mock_uname "${TEST_DIR}/mock-bin" "Linux" "x86_64"

    # Place binary at bin/markymark (no .exe)
    local mock_binary="${TEST_DIR}/bin/markymark"
    printf '#!/usr/bin/env bash\necho "LINUX_BIN"\n' > "${mock_binary}"
    chmod +x "${mock_binary}"

    local output
    output="$(PATH="${TEST_DIR}/mock-bin:${PATH}" "${TEST_DIR}/scripts/select-binary.sh" 2>&1)" || true

    if [[ "${output}" == "LINUX_BIN" ]]; then
        pass "Non-Windows binary path has no .exe suffix"
    else
        fail "Non-Windows binary path has no .exe suffix" "LINUX_BIN" "${output}"
    fi

    cleanup_test_env
}

# ─── Run all tests ───────────────────────────────────────────────
echo "=== markymark-plugin tests (bundled binary model) ==="
echo ""

test_script_exists
test_plugin_structure
test_plugin_json_valid
test_marketplace_json_valid
test_marketplace_json_fields
test_marketplace_plugin_source_exists
test_lsp_json_uses_plugin_root
test_mcp_json_uses_plugin_root
test_bundled_binary
test_forwards_arguments
test_missing_binary_attempts_download
test_download_failure_shows_manual_instructions
test_successful_download_executes
test_download_url_has_correct_target
test_error_suggests_platform_archive
test_makes_binary_executable
test_ignores_platform_specific_binaries
test_windows_binary_has_exe_suffix
test_windows_download_url_has_exe_suffix
test_non_windows_binary_no_exe_suffix

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
