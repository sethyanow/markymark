#!/usr/bin/env bash
# Tests for format-suggestions.sh — suggest-links report formatting from curation-diagnostics JSON.
#
# Run: bash markymark-plugin/tests/test_format_suggestions.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="${SCRIPT_DIR}/.."
FORMAT_SUGGESTIONS="${PLUGIN_DIR}/skills/suggest-links/scripts/format-suggestions.sh"

PASS=0
FAIL=0
TESTS_RUN=0

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

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

# ─── Fixture: Normal curation with suggestions ───────────────────
FIXTURE_NORMAL='{
  "realm": "default",
  "orphan_docs": ["file:///docs/orphan1.md", "file:///docs/orphan2.md"],
  "low_connectivity_docs": [
    {"uri": "file:///docs/sparse.md", "connectivity": 1, "in_degree": 0, "out_degree": 1}
  ],
  "suggestions": [
    {
      "source_doc": "file:///docs/index.md",
      "target_doc": "file:///docs/orphan1.md",
      "reason": "Co-located in same directory",
      "suggestion_type": "reduce_orphan"
    },
    {
      "source_doc": "file:///docs/guide.md",
      "target_doc": "file:///docs/sparse.md",
      "reason": "Both in docs directory",
      "suggestion_type": "cross_link"
    }
  ],
  "stats": {
    "total_docs": 10,
    "orphan_count": 2,
    "orphan_percentage": 20.0,
    "avg_connectivity": 3.5,
    "median_connectivity": 3.0,
    "broken_link_count": 0
  }
}'

# ─── Fixture: Empty realm ────────────────────────────────────────
FIXTURE_EMPTY='{
  "realm": "empty",
  "orphan_docs": [],
  "low_connectivity_docs": [],
  "suggestions": [],
  "stats": {
    "total_docs": 0,
    "orphan_count": 0,
    "orphan_percentage": 0.0,
    "avg_connectivity": 0.0,
    "median_connectivity": 0.0,
    "broken_link_count": 0
  }
}'

# ─── Fixture: Single-doc realm ───────────────────────────────────
FIXTURE_SINGLE='{
  "realm": "solo",
  "orphan_docs": ["file:///docs/only.md"],
  "low_connectivity_docs": [],
  "suggestions": [],
  "stats": {
    "total_docs": 1,
    "orphan_count": 1,
    "orphan_percentage": 100.0,
    "avg_connectivity": 0.0,
    "median_connectivity": 0.0,
    "broken_link_count": 0
  }
}'

# ─── Fixture: Clean realm (no orphans, no suggestions) ──────────
FIXTURE_CLEAN='{
  "realm": "healthy",
  "orphan_docs": [],
  "low_connectivity_docs": [],
  "suggestions": [],
  "stats": {
    "total_docs": 8,
    "orphan_count": 0,
    "orphan_percentage": 0.0,
    "avg_connectivity": 5.2,
    "median_connectivity": 5.0,
    "broken_link_count": 0
  }
}'

# ─── Fixture: Orphans but no suggestions (no hubs) ──────────────
FIXTURE_NO_HUBS='{
  "realm": "flat",
  "orphan_docs": ["file:///docs/a.md", "file:///docs/b.md", "file:///docs/c.md"],
  "low_connectivity_docs": [],
  "suggestions": [],
  "stats": {
    "total_docs": 3,
    "orphan_count": 3,
    "orphan_percentage": 100.0,
    "avg_connectivity": 0.0,
    "median_connectivity": 0.0,
    "broken_link_count": 0
  }
}'

# ─── Fixture: Error envelope ─────────────────────────────────────
FIXTURE_ERROR='{
  "error": {
    "code": "realm_not_found",
    "message": "Realm missing does not exist"
  }
}'

# ─── Fixture: Large output (many orphans for truncation test) ────
generate_large_fixture() {
    local orphans=""
    local orphan_docs=""
    local suggestions=""
    for i in $(seq 1 30); do
        [[ -n "${orphan_docs}" ]] && orphan_docs="${orphan_docs},"
        orphan_docs="${orphan_docs}\"file:///docs/orphan${i}.md\""
    done
    for i in $(seq 1 30); do
        [[ -n "${suggestions}" ]] && suggestions="${suggestions},"
        suggestions="${suggestions}{\"source_doc\":\"file:///docs/hub.md\",\"target_doc\":\"file:///docs/orphan${i}.md\",\"reason\":\"Co-located\",\"suggestion_type\":\"reduce_orphan\"}"
    done

    cat <<EOF
{
  "realm": "large",
  "orphan_docs": [${orphan_docs}],
  "low_connectivity_docs": [],
  "suggestions": [${suggestions}],
  "stats": {"total_docs": 50, "orphan_count": 30, "orphan_percentage": 60.0, "avg_connectivity": 1.6, "median_connectivity": 0.0, "broken_link_count": 0}
}
EOF
}

# ─── Test 1: Script exists and is executable ─────────────────────
test_script_exists() {
    if [[ -x "${FORMAT_SUGGESTIONS}" ]]; then
        pass "format-suggestions.sh exists and is executable"
    else
        fail "format-suggestions.sh exists and is executable" "executable file" "not found or not executable"
    fi
}

# ─── Test 2: Empty stdin exits 1 ─────────────────────────────────
test_empty_stdin() {
    local exit_code
    echo "" | "${FORMAT_SUGGESTIONS}" >/dev/null 2>&1 && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 1 ]]; then
        pass "empty stdin (exit 1)"
    else
        fail "empty stdin" "exit 1" "exit ${exit_code}"
    fi
}

# ─── Test 3: Malformed JSON exits 1 ──────────────────────────────
test_malformed_json() {
    local exit_code
    echo "not valid json {{{" | "${FORMAT_SUGGESTIONS}" >/dev/null 2>&1 && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 1 ]]; then
        pass "malformed JSON (exit 1)"
    else
        fail "malformed JSON" "exit 1" "exit ${exit_code}"
    fi
}

# ─── Test 4: Error envelope exits 1 ──────────────────────────────
test_error_envelope() {
    local output exit_code
    output="$(echo "${FIXTURE_ERROR}" | "${FORMAT_SUGGESTIONS}" 2>&1)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 1 ]]; then
        # Should mention the error message
        if [[ "${output}" == *"Realm missing"* ]] || [[ "${output}" == *"realm_not_found"* ]]; then
            pass "error envelope (exit 1 with error message)"
        else
            fail "error envelope" "error message in output" "${output:0:200}"
        fi
    else
        fail "error envelope" "exit 1" "exit ${exit_code}"
    fi
}

# ─── Test 5: Normal output with wiki-link syntax ─────────────────
test_normal_output() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" || {
        fail "normal output" "formatted markdown" "script failed"
        return
    }

    local ok=true

    # Must contain realm name
    [[ "${output}" == *"default"* ]] || { ok=false; fail "normal: realm name" "contains 'default'" "missing"; }

    # Must contain wiki-link syntax [[target]]
    [[ "${output}" == *"[[orphan1]]"* ]] || [[ "${output}" == *"[[orphan1.md]]"* ]] || { ok=false; fail "normal: wiki-link syntax" "contains [[orphan1]]" "missing"; }

    # Must mention source doc
    [[ "${output}" == *"index.md"* ]] || { ok=false; fail "normal: source doc" "contains 'index.md'" "missing"; }

    # Must contain suggestion reason
    [[ "${output}" == *"Co-located"* ]] || { ok=false; fail "normal: suggestion reason" "contains reason" "missing"; }

    if $ok; then
        pass "normal output (wiki-link syntax, source doc, reason)"
    fi
}

# ─── Test 6: Suggestions grouped by type ─────────────────────────
test_grouped_by_type() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" || {
        fail "grouped by type" "grouped sections" "script failed"
        return
    }

    local has_orphan_section=false
    local has_connectivity_section=false

    # Check for orphan reduction section
    [[ "${output}" == *"Orphan"* ]] || [[ "${output}" == *"orphan"* ]] && has_orphan_section=true

    # Check for connectivity section (from low_connectivity_docs or cross_link suggestions)
    [[ "${output}" == *"onnectivity"* ]] || [[ "${output}" == *"cross"* ]] || [[ "${output}" == *"Cross"* ]] && has_connectivity_section=true

    if $has_orphan_section && $has_connectivity_section; then
        pass "suggestions grouped by type (orphan + connectivity sections)"
    else
        fail "grouped by type" "both orphan and connectivity sections" "orphan=${has_orphan_section} connectivity=${has_connectivity_section}"
    fi
}

# ─── Test 7: Low-connectivity docs in output ─────────────────────
test_low_connectivity() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" || {
        fail "low connectivity" "sparse.md listed" "script failed"
        return
    }

    if [[ "${output}" == *"sparse.md"* ]]; then
        pass "low-connectivity docs listed (sparse.md)"
    else
        fail "low connectivity" "contains 'sparse.md'" "missing"
    fi
}

# ─── Test 8: Empty realm ─────────────────────────────────────────
test_empty_realm() {
    local output exit_code
    output="$(echo "${FIXTURE_EMPTY}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 0 ]]; then
        if [[ "${output}" == *"No documents"* ]] || [[ "${output}" == *"no documents"* ]] || [[ "${output}" == *"0"*"doc"* ]]; then
            pass "empty realm (clean message, exit 0)"
        else
            fail "empty realm" "mentions no documents" "${output:0:200}"
        fi
    else
        fail "empty realm" "exit 0" "exit ${exit_code}"
    fi
}

# ─── Test 9: Single-doc realm ────────────────────────────────────
test_single_doc_realm() {
    local output exit_code
    output="$(echo "${FIXTURE_SINGLE}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 0 ]]; then
        if [[ "${output}" == *"single"* ]] || [[ "${output}" == *"Single"* ]] || [[ "${output}" == *"no cross-links"* ]]; then
            pass "single-doc realm (notes no cross-links possible)"
        else
            fail "single-doc realm" "mentions single doc / no cross-links" "${output:0:200}"
        fi
    else
        fail "single-doc realm" "exit 0" "exit ${exit_code}"
    fi
}

# ─── Test 10: Orphans present but no suggestions ─────────────────
test_orphans_no_suggestions() {
    local output
    output="$(echo "${FIXTURE_NO_HUBS}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" || {
        fail "orphans no suggestions" "lists orphans" "script failed"
        return
    }

    local has_orphans=false
    local has_note=false

    # Should list orphan docs
    [[ "${output}" == *"a.md"* ]] && has_orphans=true

    # Should note that no hub targets are available
    [[ "${output}" == *"no hub"* ]] || [[ "${output}" == *"No hub"* ]] || [[ "${output}" == *"no suggestions"* ]] || [[ "${output}" == *"No suggestions"* ]] && has_note=true

    if $has_orphans && $has_note; then
        pass "orphans without suggestions (lists orphans + notes no hubs)"
    else
        fail "orphans no suggestions" "orphans listed + no-hubs note" "orphans=${has_orphans} note=${has_note}"
    fi
}

# ─── Test 11: Clean realm (no orphans, no suggestions) ───────────
test_clean_realm() {
    local output exit_code
    output="$(echo "${FIXTURE_CLEAN}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 0 ]]; then
        if [[ "${output}" == *"no suggestions"* ]] || [[ "${output}" == *"No suggestions"* ]] || [[ "${output}" == *"well-connected"* ]] || [[ "${output}" == *"healthy"* ]]; then
            pass "clean realm (no suggestions needed)"
        else
            fail "clean realm" "indicates no suggestions needed" "${output:0:200}"
        fi
    else
        fail "clean realm" "exit 0" "exit ${exit_code}"
    fi
}

# ─── Test 12: --max-items truncation ─────────────────────────────
test_max_items_truncation() {
    local output
    output="$(generate_large_fixture | "${FORMAT_SUGGESTIONS}" --max-items 5 2>/dev/null)" || {
        fail "max-items truncation" "truncated output" "script failed"
        return
    }

    if [[ "${output}" == *"and "* ]] && [[ "${output}" == *"more"* ]]; then
        pass "max-items truncation (shows '... and N more')"
    else
        fail "max-items truncation" "truncation message" "${output:0:300}"
    fi
}

# ─── Test 13: URI shortening ─────────────────────────────────────
test_uri_shortening() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_SUGGESTIONS}" 2>/dev/null)" || {
        fail "URI shortening" "short paths" "script failed"
        return
    }

    if [[ "${output}" != *"file:///"* ]]; then
        pass "URI shortening (no raw file:// URIs in output)"
    else
        fail "URI shortening" "shortened paths" "found file:// URIs"
    fi
}

# ─── Run all tests ───────────────────────────────────────────────
echo "=== suggest-links format-suggestions.sh tests ==="
echo ""

test_script_exists
test_empty_stdin
test_malformed_json
test_error_envelope
test_normal_output
test_grouped_by_type
test_low_connectivity
test_empty_realm
test_single_doc_realm
test_orphans_no_suggestions
test_clean_realm
test_max_items_truncation
test_uri_shortening

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
