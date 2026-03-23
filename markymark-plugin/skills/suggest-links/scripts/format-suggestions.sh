#!/usr/bin/env bash
set -euo pipefail

# format-suggestions.sh — Format curation-diagnostics JSON into actionable link suggestions.
#
# Usage: echo '{"realm":...,"suggestions":...}' | format-suggestions.sh [--max-items N]
#
# Input:  curation-diagnostics MCP tool JSON on stdin.
#         An error envelope is detected by the presence of an "error" key.
#
# Output: Formatted markdown with concrete wiki-link syntax on stdout.
#
# Exit codes:
#   0 = success
#   1 = invalid input (malformed JSON, empty stdin, error envelope)

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

# Detect error envelope
if echo "${input}" | jq -e '.error' >/dev/null 2>&1; then
    error_msg="$(echo "${input}" | jq -r '.error.message // .error.code // "unknown error"')"
    echo "Error: curation-diagnostics failed — ${error_msg}" >&2
    exit 1
fi

# Extract data
realm="$(echo "${input}" | jq -r '.realm // "unknown"')"
total_docs="$(echo "${input}" | jq -r '.stats.total_docs // 0')"
orphan_count="$(echo "${input}" | jq -r '.stats.orphan_count // 0')"
orphan_pct="$(echo "${input}" | jq -r '.stats.orphan_percentage // 0')"
avg_conn="$(echo "${input}" | jq -r '.stats.avg_connectivity // 0')"
suggestion_count="$(echo "${input}" | jq '.suggestions | length')"
low_conn_count="$(echo "${input}" | jq '.low_connectivity_docs | length')"

# Helper: extract wiki-link stem from file URI (filename without extension)
wiki_stem() {
    local uri="$1"
    local basename
    basename="$(echo "${uri}" | sed 's|^file://||; s|.*/||')"
    echo "${basename}" | sed 's|\.[^.]*$||'
}

# ─── Report Header ──────────────────────────────────────────────
echo "# Link Suggestions"
echo ""
echo "**Realm:** ${realm} | **Documents:** ${total_docs} | **Orphans:** ${orphan_count} (${orphan_pct}%)"

# ─── Special cases ──────────────────────────────────────────────
if [[ ${total_docs} -eq 0 ]]; then
    echo ""
    echo "No documents indexed. Nothing to suggest."
    exit 0
fi

if [[ ${total_docs} -eq 1 ]]; then
    echo ""
    echo "> **Note:** Single document in realm — no cross-links possible."
    exit 0
fi

# Check if there's anything to suggest
if [[ ${orphan_count} -eq 0 ]] && [[ ${suggestion_count} -eq 0 ]] && [[ ${low_conn_count} -eq 0 ]]; then
    echo ""
    echo "No suggestions needed — documentation is well-connected (avg connectivity: ${avg_conn})."
    exit 0
fi

# ─── Reduce Orphans section ─────────────────────────────────────
reduce_orphan_suggestions="$(echo "${input}" | jq '[.suggestions[] | select(.suggestion_type == "reduce_orphan")]')"
reduce_orphan_count="$(echo "${reduce_orphan_suggestions}" | jq 'length')"

if [[ ${orphan_count} -gt 0 ]]; then
    echo ""
    echo "## Reduce Orphans"
    echo ""

    if [[ ${reduce_orphan_count} -gt 0 ]]; then
        echo "These orphan documents can be linked to existing hubs:"
        echo ""

        shown=0
        echo "${reduce_orphan_suggestions}" | jq -c '.[]' | while read -r sug; do
            if [[ ${shown} -ge ${MAX_ITEMS} ]]; then
                remaining=$((reduce_orphan_count - MAX_ITEMS))
                echo "- ... and ${remaining} more (use --max-items to see all)"
                break
            fi
            source_uri="$(echo "${sug}" | jq -r '.source_doc')"
            target_uri="$(echo "${sug}" | jq -r '.target_doc')"
            reason="$(echo "${sug}" | jq -r '.reason')"
            source_path="$(echo "${source_uri}" | sed 's|^file://||')"
            target_stem="$(wiki_stem "${target_uri}")"
            echo "- In \`${source_path}\`, add \`[[${target_stem}]]\` — ${reason}"
            shown=$((shown + 1))
        done
    else
        echo "No hub targets available for automatic suggestions. Orphan documents:"
        echo ""
        orphan_shown=0
        echo "${input}" | jq -r '.orphan_docs[]' | while read -r orphan_uri; do
            if [[ ${orphan_shown} -ge ${MAX_ITEMS} ]]; then
                remaining=$((orphan_count - MAX_ITEMS))
                echo "- ... and ${remaining} more"
                break
            fi
            echo "- $(echo "${orphan_uri}" | sed 's|^file://||')"
            orphan_shown=$((orphan_shown + 1))
        done
        echo ""
        echo "> **Tip:** Add these documents to an index page or create cross-references manually."
    fi
fi

# ─── Cross-Link section ─────────────────────────────────────────
cross_link_suggestions="$(echo "${input}" | jq '[.suggestions[] | select(.suggestion_type == "cross_link")]')"
cross_link_count="$(echo "${cross_link_suggestions}" | jq 'length')"

if [[ ${cross_link_count} -gt 0 ]]; then
    echo ""
    echo "## Improve Cross-Linking"
    echo ""
    echo "These documents would benefit from additional connections:"
    echo ""

    shown=0
    echo "${cross_link_suggestions}" | jq -c '.[]' | while read -r sug; do
        if [[ ${shown} -ge ${MAX_ITEMS} ]]; then
            remaining=$((cross_link_count - MAX_ITEMS))
            echo "- ... and ${remaining} more (use --max-items to see all)"
            break
        fi
        source_uri="$(echo "${sug}" | jq -r '.source_doc')"
        target_uri="$(echo "${sug}" | jq -r '.target_doc')"
        reason="$(echo "${sug}" | jq -r '.reason')"
        source_path="$(echo "${source_uri}" | sed 's|^file://||')"
        target_stem="$(wiki_stem "${target_uri}")"
        echo "- In \`${source_path}\`, add \`[[${target_stem}]]\` — ${reason}"
        shown=$((shown + 1))
    done
fi

# ─── Low Connectivity section ────────────────────────────────────
if [[ ${low_conn_count} -gt 0 ]]; then
    echo ""
    echo "## Low Connectivity"
    echo ""
    echo "These documents have below-median connectivity and would benefit from more links:"
    echo ""

    shown=0
    echo "${input}" | jq -c '.low_connectivity_docs[]' | while read -r doc; do
        if [[ ${shown} -ge ${MAX_ITEMS} ]]; then
            remaining=$((low_conn_count - MAX_ITEMS))
            echo "- ... and ${remaining} more (use --max-items to see all)"
            break
        fi
        uri="$(echo "${doc}" | jq -r '.uri | sub("^file://"; "")')"
        conn="$(echo "${doc}" | jq -r '.connectivity')"
        in_deg="$(echo "${doc}" | jq -r '.in_degree')"
        out_deg="$(echo "${doc}" | jq -r '.out_degree')"
        echo "- \`${uri}\` (connectivity: ${conn}, in: ${in_deg}, out: ${out_deg})"
        shown=$((shown + 1))
    done
fi

# ─── Caveat ──────────────────────────────────────────────────────
echo ""
echo "> **Note:** Suggestions don't check existing links — verify before adding duplicates."

exit 0
