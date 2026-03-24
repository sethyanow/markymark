#!/usr/bin/env bash
set -euo pipefail

# format-report.sh — Format MCP tool JSON into a markdown documentation audit report.
#
# Usage: echo '{"graph_analysis":...,"diagnostics":...,"curation":...}' | format-report.sh [--max-items N]
#
# Input:  JSON on stdin with keys: graph_analysis, diagnostics, curation
#         Each key holds the response from the corresponding MCP tool.
#         A tool error is detected by the presence of an "error" key.
#
# Output: Formatted markdown report on stdout.
#
# Exit codes:
#   0 = all tools succeeded
#   1 = invalid input (malformed JSON, empty stdin)
#   2 = partial success (some tools failed)

MAX_ITEMS=20

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --max-items)
            MAX_ITEMS="${2:-20}"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

# Read stdin
input="$(cat)"
if [[ -z "${input}" ]] || [[ "${input}" =~ ^[[:space:]]*$ ]]; then
    echo "Error: empty input on stdin" >&2
    exit 1
fi

# Validate JSON
if ! echo "${input}" | jq empty 2>/dev/null; then
    echo "Error: malformed JSON input" >&2
    exit 1
fi

# Detect which tools succeeded vs failed
tool_status_graph="ok"
tool_status_diag="ok"
tool_status_curation="ok"
failed_count=0

if echo "${input}" | jq -e '.graph_analysis.error' >/dev/null 2>&1; then
    tool_status_graph="failed"
    failed_count=$((failed_count + 1))
fi
if echo "${input}" | jq -e '.diagnostics.error' >/dev/null 2>&1; then
    tool_status_diag="failed"
    failed_count=$((failed_count + 1))
fi
if echo "${input}" | jq -e '.curation.error' >/dev/null 2>&1; then
    tool_status_curation="failed"
    failed_count=$((failed_count + 1))
fi

# If all three failed, exit 1
if [[ ${failed_count} -eq 3 ]]; then
    echo "Error: all tools failed" >&2
    exit 1
fi

# Extract realm name from first successful tool
realm="unknown"
if [[ "${tool_status_graph}" == "ok" ]]; then
    realm="$(echo "${input}" | jq -r '.graph_analysis.realm // "unknown"')"
elif [[ "${tool_status_curation}" == "ok" ]]; then
    realm="$(echo "${input}" | jq -r '.curation.realm // "unknown"')"
elif [[ "${tool_status_diag}" == "ok" ]]; then
    realm="$(echo "${input}" | jq -r '.diagnostics.realm // "unknown"')"
fi

# Extract stats
total_docs=0
total_links=0
orphan_count=0
broken_link_count=0

if [[ "${tool_status_graph}" == "ok" ]]; then
    total_docs="$(echo "${input}" | jq -r '.graph_analysis.stats.total_docs // 0')"
    total_links="$(echo "${input}" | jq -r '.graph_analysis.stats.total_internal_links // 0')"
    orphan_count="$(echo "${input}" | jq -r '.graph_analysis.stats.orphan_count // 0')"
    broken_link_count="$(echo "${input}" | jq -r '.graph_analysis.stats.broken_link_count // 0')"
elif [[ "${tool_status_curation}" == "ok" ]]; then
    total_docs="$(echo "${input}" | jq -r '.curation.stats.total_docs // 0')"
    orphan_count="$(echo "${input}" | jq -r '.curation.stats.orphan_count // 0')"
    broken_link_count="$(echo "${input}" | jq -r '.curation.stats.broken_link_count // 0')"
fi

# Helper: shorten file:// URIs to relative paths
shorten_uri() {
    local uri="$1"
    echo "${uri}" | sed 's|^file://||'
}

# Helper: print a list with truncation
# Usage: print_list <jq_filter> <label>
print_items() {
    local items_json="$1"
    local count
    count="$(echo "${items_json}" | jq 'length')"

    if [[ ${count} -eq 0 ]]; then
        return
    fi

    local show=${MAX_ITEMS}
    if [[ ${count} -le ${show} ]]; then
        echo "${items_json}" | jq -r '.[]'
    else
        echo "${items_json}" | jq -r ".[0:${show}][]"
        local remaining=$((count - show))
        echo "... and ${remaining} more (use --max-items to see all)"
    fi
}

# ─── Report Header ──────────────────────────────────────────────
echo "# Documentation Audit Report"
echo ""
echo "**Realm:** ${realm} | **Documents:** ${total_docs} | **Internal links:** ${total_links}"

# Tool status indicators
if [[ ${failed_count} -gt 0 ]]; then
    echo ""
    echo "> **WARNING: Partial results.** ${failed_count} tool(s) failed."
    [[ "${tool_status_graph}" == "failed" ]] && echo "> - graph-analysis: FAILED — $(echo "${input}" | jq -r '.graph_analysis.error.message // "unknown error"')"
    [[ "${tool_status_diag}" == "failed" ]] && echo "> - get-diagnostics: FAILED — $(echo "${input}" | jq -r '.diagnostics.error.message // "unknown error"')"
    [[ "${tool_status_curation}" == "failed" ]] && echo "> - curation-diagnostics: FAILED — $(echo "${input}" | jq -r '.curation.error.message // "unknown error"')"
fi

# ─── Special cases ──────────────────────────────────────────────
if [[ ${total_docs} -eq 0 ]]; then
    echo ""
    echo "No documents indexed. Nothing to audit."
    [[ ${failed_count} -gt 0 ]] && exit 2
    exit 0
fi

if [[ ${total_docs} -eq 1 ]]; then
    echo ""
    echo "> **Note:** Single-document corpus — orphan status is expected."
fi

# ─── Summary ────────────────────────────────────────────────────
echo ""
echo "## Summary"
echo ""
echo "| Metric | Count |"
echo "|--------|-------|"
echo "| Documents | ${total_docs} |"
echo "| Internal links | ${total_links} |"
echo "| Orphan documents | ${orphan_count} |"
echo "| Broken links | ${broken_link_count} |"

if [[ "${tool_status_curation}" == "ok" ]]; then
    avg_conn="$(echo "${input}" | jq -r '.curation.stats.avg_connectivity // 0')"
    median_conn="$(echo "${input}" | jq -r '.curation.stats.median_connectivity // 0')"
    echo "| Avg connectivity | ${avg_conn} |"
    echo "| Median connectivity | ${median_conn} |"
fi

if [[ "${tool_status_diag}" == "ok" ]]; then
    files_with_issues="$(echo "${input}" | jq -r '.diagnostics.files_with_issues // 0')"
    echo "| Files with diagnostics | ${files_with_issues} |"
fi

# Check if healthy
if [[ ${orphan_count} -eq 0 ]] && [[ ${broken_link_count} -eq 0 ]]; then
    has_diag_issues=false
    if [[ "${tool_status_diag}" == "ok" ]]; then
        diag_count="$(echo "${input}" | jq '[.diagnostics.diagnostics[].diagnostics[]] | length')"
        [[ ${diag_count} -gt 0 ]] && has_diag_issues=true
    fi
    if ! $has_diag_issues; then
        echo ""
        echo "No orphan documents, no broken links, no diagnostics issues. Documentation is healthy."
    fi
fi

# ─── Orphan Documents ──────────────────────────────────────────
if [[ "${tool_status_graph}" == "ok" ]] && [[ ${orphan_count} -gt 0 ]]; then
    echo ""
    echo "## Orphan Documents"
    echo ""
    echo "Documents with no incoming or outgoing links:"
    echo ""
    orphan_lines="$(echo "${input}" | jq -r "[.graph_analysis.orphans[].uri | sub(\"^file://\"; \"\")] | .[]" | while read -r uri; do
        echo "- ${uri}"
    done)"
    # Truncation
    orphan_total="$(echo "${input}" | jq '.graph_analysis.orphans | length')"
    if [[ ${orphan_total} -le ${MAX_ITEMS} ]]; then
        echo "${orphan_lines}"
    else
        echo "${orphan_lines}" | head -n "${MAX_ITEMS}"
        remaining=$((orphan_total - MAX_ITEMS))
        echo "- ... and ${remaining} more (use --max-items to see all)"
    fi
    echo ""
    echo "**Action:** Add links to or from these documents, or remove them if obsolete."
fi

# ─── Broken Links ──────────────────────────────────────────────
if [[ "${tool_status_graph}" == "ok" ]] && [[ ${broken_link_count} -gt 0 ]]; then
    echo ""
    echo "## Broken Links"
    echo ""
    echo "Links that could not be resolved:"
    echo ""
    broken_lines="$(echo "${input}" | jq -r '.graph_analysis.broken_links[] | "- [\(.kind)] \(.source_uri | sub("^file://"; "")) → \(.target)"')"
    broken_total="$(echo "${input}" | jq '.graph_analysis.broken_links | length')"
    if [[ ${broken_total} -le ${MAX_ITEMS} ]]; then
        echo "${broken_lines}"
    else
        echo "${broken_lines}" | head -n "${MAX_ITEMS}"
        remaining=$((broken_total - MAX_ITEMS))
        echo "- ... and ${remaining} more (use --max-items to see all)"
    fi
    echo ""
    echo "**Action:** Fix targets or remove dead links."
fi

# ─── Hub Documents ─────────────────────────────────────────────
if [[ "${tool_status_graph}" == "ok" ]]; then
    hub_count="$(echo "${input}" | jq '.graph_analysis.hubs | length')"
    if [[ ${hub_count} -gt 0 ]]; then
        echo ""
        echo "## Hub Documents"
        echo ""
        echo "Most-linked documents (highest incoming link count):"
        echo ""
        echo "${input}" | jq -r ".graph_analysis.hubs[:${MAX_ITEMS}][] | \"- \(.uri | sub(\"^file://\"; \"\")) (\(.incoming_count) incoming)\""
        if [[ ${hub_count} -gt ${MAX_ITEMS} ]]; then
            remaining=$((hub_count - MAX_ITEMS))
            echo "- ... and ${remaining} more (use --max-items to see all)"
        fi
    fi
fi

# ─── Low Connectivity ──────────────────────────────────────────
if [[ "${tool_status_curation}" == "ok" ]]; then
    low_conn_count="$(echo "${input}" | jq '.curation.low_connectivity_docs | length')"
    if [[ ${low_conn_count} -gt 0 ]]; then
        echo ""
        echo "## Low Connectivity"
        echo ""
        echo "Documents below median connectivity:"
        echo ""
        echo "${input}" | jq -r ".curation.low_connectivity_docs[:${MAX_ITEMS}][] | \"- \(.uri | sub(\"^file://\"; \"\")) (connectivity: \(.connectivity), in: \(.in_degree), out: \(.out_degree))\""
        if [[ ${low_conn_count} -gt ${MAX_ITEMS} ]]; then
            remaining=$((low_conn_count - MAX_ITEMS))
            echo "- ... and ${remaining} more (use --max-items to see all)"
        fi
        echo ""
        echo "**Action:** Add cross-references to improve connectivity."
    fi
fi

# ─── Suggestions ────────────────────────────────────────────────
if [[ "${tool_status_curation}" == "ok" ]]; then
    suggestion_count="$(echo "${input}" | jq '.curation.suggestions | length')"
    if [[ ${suggestion_count} -gt 0 ]]; then
        echo ""
        echo "## Suggested Links"
        echo ""
        echo "${input}" | jq -r ".curation.suggestions[:${MAX_ITEMS}][] | \"- \(.source_doc | sub(\"^file://\"; \"\")) → \(.target_doc | sub(\"^file://\"; \"\")): \(.reason) [\(.suggestion_type)]\""
        if [[ ${suggestion_count} -gt ${MAX_ITEMS} ]]; then
            remaining=$((suggestion_count - MAX_ITEMS))
            echo "- ... and ${remaining} more (use --max-items to see all)"
        fi
    fi
fi

# ─── Diagnostics ───────────────────────────────────────────────
if [[ "${tool_status_diag}" == "ok" ]]; then
    diag_file_count="$(echo "${input}" | jq '.diagnostics.diagnostics | length')"
    if [[ ${diag_file_count} -gt 0 ]]; then
        echo ""
        echo "## File Diagnostics"
        echo ""
        shown=0
        echo "${input}" | jq -c '.diagnostics.diagnostics[]' | while read -r file_diag; do
            if [[ ${shown} -ge ${MAX_ITEMS} ]]; then
                remaining=$((diag_file_count - MAX_ITEMS))
                echo "... and ${remaining} more files (use --max-items to see all)"
                break
            fi
            uri="$(echo "${file_diag}" | jq -r '.uri | sub("^file://"; "")')"
            echo "### ${uri}"
            echo ""
            echo "${file_diag}" | jq -r '.diagnostics[] | "- **\(.severity)** (L\(.range.start.line)): \(.message)"'
            echo ""
            shown=$((shown + 1))
        done
    fi
fi

# Exit code
if [[ ${failed_count} -gt 0 ]]; then
    exit 2
fi
exit 0
