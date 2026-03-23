#!/usr/bin/env bash
# Tests for format-report.sh — doc-audit report formatting from MCP tool JSON.
#
# Run: bash markymark-plugin/tests/test_format_report.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="${SCRIPT_DIR}/.."
FORMAT_REPORT="${PLUGIN_DIR}/skills/doc-audit/scripts/format-report.sh"

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

# ─── Fixture: Normal realm with issues ──────────────────────────
FIXTURE_NORMAL='{
  "graph_analysis": {
    "realm": "default",
    "stats": {
      "total_docs": 12,
      "total_internal_links": 30,
      "orphan_count": 2,
      "broken_link_count": 3,
      "cluster_count": null
    },
    "orphans": [
      {"uri": "file:///docs/orphan1.md"},
      {"uri": "file:///docs/orphan2.md"}
    ],
    "hubs": [
      {"uri": "file:///docs/index.md", "incoming_count": 8},
      {"uri": "file:///docs/guide.md", "incoming_count": 5}
    ],
    "broken_links": [
      {"source_uri": "file:///docs/intro.md", "target": "missing-page", "kind": "wiki"},
      {"source_uri": "file:///docs/guide.md", "target": "./gone.md", "kind": "markdown"},
      {"source_uri": "file:///docs/ref.md", "target": "also-missing", "kind": "wiki"}
    ],
    "clusters": null
  },
  "diagnostics": {
    "realm": "default",
    "files_with_issues": 2,
    "diagnostics": [
      {
        "uri": "file:///docs/intro.md",
        "diagnostics": [
          {
            "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 20}},
            "severity": "warning",
            "message": "Duplicate heading: Introduction"
          }
        ]
      },
      {
        "uri": "file:///docs/ref.md",
        "diagnostics": [
          {
            "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 15}},
            "severity": "error",
            "message": "Broken link: also-missing"
          },
          {
            "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 10}},
            "severity": "warning",
            "message": "Unclosed XML tag: <details>"
          }
        ]
      }
    ]
  },
  "curation": {
    "realm": "default",
    "orphan_docs": ["file:///docs/orphan1.md", "file:///docs/orphan2.md"],
    "low_connectivity_docs": [
      {"uri": "file:///docs/appendix.md", "connectivity": 1, "in_degree": 0, "out_degree": 1}
    ],
    "suggestions": [
      {
        "source_doc": "file:///docs/index.md",
        "target_doc": "file:///docs/orphan1.md",
        "reason": "Co-located in same directory",
        "suggestion_type": "reduce_orphan"
      }
    ],
    "stats": {
      "total_docs": 12,
      "orphan_count": 2,
      "orphan_percentage": 16.7,
      "avg_connectivity": 5.0,
      "median_connectivity": 4.0,
      "broken_link_count": 3
    }
  }
}'

# ─── Fixture: Empty realm ───────────────────────────────────────
FIXTURE_EMPTY='{
  "graph_analysis": {
    "realm": "empty-realm",
    "stats": {"total_docs": 0, "total_internal_links": 0, "orphan_count": 0, "broken_link_count": 0, "cluster_count": null},
    "orphans": [],
    "hubs": [],
    "broken_links": [],
    "clusters": null
  },
  "diagnostics": {
    "realm": "empty-realm",
    "files_with_issues": 0,
    "diagnostics": []
  },
  "curation": {
    "realm": "empty-realm",
    "orphan_docs": [],
    "low_connectivity_docs": [],
    "suggestions": [],
    "stats": {"total_docs": 0, "orphan_count": 0, "orphan_percentage": 0.0, "avg_connectivity": 0.0, "median_connectivity": 0.0, "broken_link_count": 0}
  }
}'

# ─── Fixture: Single-doc realm ──────────────────────────────────
FIXTURE_SINGLE='{
  "graph_analysis": {
    "realm": "single",
    "stats": {"total_docs": 1, "total_internal_links": 0, "orphan_count": 1, "broken_link_count": 0, "cluster_count": null},
    "orphans": [{"uri": "file:///docs/only.md"}],
    "hubs": [],
    "broken_links": [],
    "clusters": null
  },
  "diagnostics": {
    "realm": "single",
    "files_with_issues": 0,
    "diagnostics": []
  },
  "curation": {
    "realm": "single",
    "orphan_docs": ["file:///docs/only.md"],
    "low_connectivity_docs": [],
    "suggestions": [],
    "stats": {"total_docs": 1, "orphan_count": 1, "orphan_percentage": 100.0, "avg_connectivity": 0.0, "median_connectivity": 0.0, "broken_link_count": 0}
  }
}'

# ─── Fixture: Clean realm (no issues) ──────────────────────────
FIXTURE_CLEAN='{
  "graph_analysis": {
    "realm": "healthy",
    "stats": {"total_docs": 5, "total_internal_links": 12, "orphan_count": 0, "broken_link_count": 0, "cluster_count": null},
    "orphans": [],
    "hubs": [{"uri": "file:///docs/index.md", "incoming_count": 4}],
    "broken_links": [],
    "clusters": null
  },
  "diagnostics": {
    "realm": "healthy",
    "files_with_issues": 0,
    "diagnostics": []
  },
  "curation": {
    "realm": "healthy",
    "orphan_docs": [],
    "low_connectivity_docs": [],
    "suggestions": [],
    "stats": {"total_docs": 5, "orphan_count": 0, "orphan_percentage": 0.0, "avg_connectivity": 4.8, "median_connectivity": 4.0, "broken_link_count": 0}
  }
}'

# ─── Fixture: Partial failure (diagnostics tool failed) ────────
FIXTURE_PARTIAL='{
  "graph_analysis": {
    "realm": "partial",
    "stats": {"total_docs": 3, "total_internal_links": 4, "orphan_count": 1, "broken_link_count": 0, "cluster_count": null},
    "orphans": [{"uri": "file:///docs/lone.md"}],
    "hubs": [{"uri": "file:///docs/main.md", "incoming_count": 2}],
    "broken_links": [],
    "clusters": null
  },
  "diagnostics": {
    "error": {"code": "realm_not_found", "message": "Realm partial does not exist"}
  },
  "curation": {
    "realm": "partial",
    "orphan_docs": ["file:///docs/lone.md"],
    "low_connectivity_docs": [],
    "suggestions": [],
    "stats": {"total_docs": 3, "orphan_count": 1, "orphan_percentage": 33.3, "avg_connectivity": 2.7, "median_connectivity": 2.0, "broken_link_count": 0}
  }
}'

# ─── Fixture: Large output (many orphans for truncation test) ──
generate_large_fixture() {
    local orphans=""
    local orphan_docs=""
    for i in $(seq 1 30); do
        [[ -n "${orphans}" ]] && orphans="${orphans},"
        orphans="${orphans}{\"uri\":\"file:///docs/orphan${i}.md\"}"
        [[ -n "${orphan_docs}" ]] && orphan_docs="${orphan_docs},"
        orphan_docs="${orphan_docs}\"file:///docs/orphan${i}.md\""
    done

    cat <<EOF
{
  "graph_analysis": {
    "realm": "large",
    "stats": {"total_docs": 50, "total_internal_links": 40, "orphan_count": 30, "broken_link_count": 0, "cluster_count": null},
    "orphans": [${orphans}],
    "hubs": [{"uri": "file:///docs/hub.md", "incoming_count": 10}],
    "broken_links": [],
    "clusters": null
  },
  "diagnostics": {
    "realm": "large",
    "files_with_issues": 0,
    "diagnostics": []
  },
  "curation": {
    "realm": "large",
    "orphan_docs": [${orphan_docs}],
    "low_connectivity_docs": [],
    "suggestions": [],
    "stats": {"total_docs": 50, "orphan_count": 30, "orphan_percentage": 60.0, "avg_connectivity": 1.6, "median_connectivity": 0.0, "broken_link_count": 0}
  }
}
EOF
}

# ─── Test: Script exists and is executable ──────────────────────
test_script_exists() {
    if [[ -x "${FORMAT_REPORT}" ]]; then
        pass "format-report.sh exists and is executable"
    else
        fail "format-report.sh exists and is executable" "executable file" "not found or not executable"
    fi
}

# ─── Test: Normal report output ─────────────────────────────────
test_normal_report() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "normal report" "formatted markdown" "script failed"
        return
    }

    local ok=true

    # Must contain realm name
    [[ "${output}" == *"default"* ]] || { ok=false; fail "normal report: realm name" "contains 'default'" "missing"; }

    # Must contain summary stats
    [[ "${output}" == *"12"* ]] || { ok=false; fail "normal report: doc count" "contains '12'" "missing"; }

    # Must contain orphans section
    [[ "${output}" == *"orphan1.md"* ]] || { ok=false; fail "normal report: orphan" "contains 'orphan1.md'" "missing"; }

    # Must contain broken links section
    [[ "${output}" == *"missing-page"* ]] || { ok=false; fail "normal report: broken link" "contains 'missing-page'" "missing"; }

    # Must contain hub documents
    [[ "${output}" == *"index.md"* ]] && [[ "${output}" == *"8"* ]] || { ok=false; fail "normal report: hubs" "contains 'index.md' with 8 links" "missing"; }

    # Must contain low connectivity
    [[ "${output}" == *"appendix.md"* ]] || { ok=false; fail "normal report: low connectivity" "contains 'appendix.md'" "missing"; }

    # Must contain suggestions
    [[ "${output}" == *"Co-located"* ]] || { ok=false; fail "normal report: suggestions" "contains suggestion reason" "missing"; }

    # Must contain diagnostics
    [[ "${output}" == *"Duplicate heading"* ]] || { ok=false; fail "normal report: diagnostic" "contains 'Duplicate heading'" "missing"; }

    if $ok; then
        pass "normal report (all sections present)"
    fi
}

# ─── Test: Empty realm produces clean report ────────────────────
test_empty_realm() {
    local output
    output="$(echo "${FIXTURE_EMPTY}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "empty realm" "clean report" "script failed"
        return
    }

    # Should NOT produce an error
    # Should mention 0 docs
    if [[ "${output}" == *"0"*"doc"* ]] || [[ "${output}" == *"empty"* ]] || [[ "${output}" == *"No documents"* ]]; then
        pass "empty realm (clean report, not error)"
    else
        fail "empty realm" "mentions 0 docs or empty" "${output:0:200}"
    fi
}

# ─── Test: Single-doc realm note ────────────────────────────────
test_single_doc_realm() {
    local output
    output="$(echo "${FIXTURE_SINGLE}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "single-doc realm" "report with note" "script failed"
        return
    }

    # Should mention single document corpus
    if [[ "${output}" == *"single"* ]] || [[ "${output}" == *"1"*"doc"* ]]; then
        pass "single-doc realm (noted in report)"
    else
        fail "single-doc realm" "mentions single doc" "${output:0:200}"
    fi
}

# ─── Test: Clean realm (no issues) ─────────────────────────────
test_clean_realm() {
    local output
    output="$(echo "${FIXTURE_CLEAN}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "clean realm" "clean bill of health" "script failed"
        return
    }

    # Should indicate no issues or healthy
    local has_no_orphans=false
    local has_no_broken=false
    [[ "${output}" == *"0 orphan"* ]] || [[ "${output}" == *"No orphan"* ]] || [[ "${output}" == *"no orphan"* ]] && has_no_orphans=true
    [[ "${output}" == *"0 broken"* ]] || [[ "${output}" == *"No broken"* ]] || [[ "${output}" == *"no broken"* ]] && has_no_broken=true

    if $has_no_orphans || $has_no_broken; then
        pass "clean realm (reports healthy state)"
    else
        fail "clean realm" "indicates no issues" "${output:0:200}"
    fi
}

# ─── Test: Malformed JSON exits 1 ──────────────────────────────
test_malformed_json() {
    local exit_code
    echo "this is not json {{{" | "${FORMAT_REPORT}" >/dev/null 2>&1 && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 1 ]]; then
        pass "malformed JSON (exit 1)"
    else
        fail "malformed JSON" "exit 1" "exit ${exit_code}"
    fi
}

# ─── Test: Empty stdin exits 1 ─────────────────────────────────
test_empty_stdin() {
    local exit_code
    echo "" | "${FORMAT_REPORT}" >/dev/null 2>&1 && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 1 ]]; then
        pass "empty stdin (exit 1)"
    else
        fail "empty stdin" "exit 1" "exit ${exit_code}"
    fi
}

# ─── Test: Partial tool failure produces warning ────────────────
test_partial_failure() {
    local output exit_code
    output="$(echo "${FIXTURE_PARTIAL}" | "${FORMAT_REPORT}" 2>/dev/null)" && exit_code=0 || exit_code=$?

    # Should exit 2 for partial success
    if [[ ${exit_code} -ne 2 ]]; then
        fail "partial failure: exit code" "exit 2" "exit ${exit_code}"
        return
    fi

    # Should contain warning about failed tool
    if [[ "${output}" == *"warning"* ]] || [[ "${output}" == *"WARNING"* ]] || [[ "${output}" == *"failed"* ]] || [[ "${output}" == *"FAILED"* ]]; then
        pass "partial failure (exit 2 with warning)"
    else
        fail "partial failure" "warning about failed tool" "${output:0:200}"
    fi
}

# ─── Test: --max-items truncation ───────────────────────────────
test_max_items_truncation() {
    local output
    output="$(generate_large_fixture | "${FORMAT_REPORT}" --max-items 5 2>/dev/null)" || {
        fail "max-items truncation" "truncated output" "script failed"
        return
    }

    # Should contain truncation message
    if [[ "${output}" == *"and "* ]] && [[ "${output}" == *"more"* ]]; then
        pass "max-items truncation (shows '... and N more')"
    else
        fail "max-items truncation" "truncation message like '... and N more'" "${output:0:300}"
    fi
}

# ─── Test: Default max-items caps output ────────────────────────
test_default_max_items() {
    local output
    output="$(generate_large_fixture | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "default max-items" "capped output" "script failed"
        return
    }

    # With 30 orphans, default cap of 20 should truncate
    if [[ "${output}" == *"and "* ]] && [[ "${output}" == *"more"* ]]; then
        pass "default max-items (caps at 20)"
    else
        fail "default max-items" "truncation at 20 items" "${output:0:300}"
    fi
}

# ─── Test: Report header has tool status ────────────────────────
test_report_header() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "report header" "header with realm and stats" "script failed"
        return
    }

    # Header should have realm name and doc count
    local first_10_lines
    first_10_lines="$(echo "${output}" | head -10)"

    if [[ "${first_10_lines}" == *"default"* ]] && [[ "${first_10_lines}" == *"12"* ]]; then
        pass "report header (realm + doc count in first 10 lines)"
    else
        fail "report header" "realm and doc count in header" "${first_10_lines}"
    fi
}

# ─── Test: URI paths are shortened in output ────────────────────
test_uri_shortening() {
    local output
    output="$(echo "${FIXTURE_NORMAL}" | "${FORMAT_REPORT}" 2>/dev/null)" || {
        fail "URI shortening" "short paths" "script failed"
        return
    }

    # Should show shortened paths, not full file:// URIs
    if [[ "${output}" != *"file:///"* ]]; then
        pass "URI shortening (no raw file:// URIs in output)"
    else
        fail "URI shortening" "shortened paths (no file:// prefix)" "found file:// URIs"
    fi
}

# ─── Test: Exit code 0 for full success ─────────────────────────
test_exit_code_success() {
    local exit_code
    echo "${FIXTURE_NORMAL}" | "${FORMAT_REPORT}" >/dev/null 2>&1 && exit_code=0 || exit_code=$?

    if [[ ${exit_code} -eq 0 ]]; then
        pass "exit code 0 for full success"
    else
        fail "exit code" "0" "${exit_code}"
    fi
}

# ─── Run all tests ──────────────────────────────────────────────
echo "=== doc-audit format-report.sh tests ==="
echo ""

test_script_exists
test_malformed_json
test_empty_stdin
test_exit_code_success
test_normal_report
test_report_header
test_uri_shortening
test_empty_realm
test_single_doc_realm
test_clean_realm
test_partial_failure
test_max_items_truncation
test_default_max_items

echo ""
echo "─────────────────────────────"
echo "Results: ${PASS} passed, ${FAIL} failed (${TESTS_RUN} total)"

if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
