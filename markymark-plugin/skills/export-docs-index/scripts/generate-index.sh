#!/usr/bin/env bash
set -euo pipefail

# generate-index.sh — Generate pipe-delimited docs_index from directory tree
#
# Usage: generate-index.sh <directory-path> <section-name> [instruction-text]
#
# Output: Pipe-delimited docs_index line(s) to stdout.
#         Errors and warnings to stderr.
#
# Limitations:
#   - Symlinks are not followed (prevents infinite loops)
#   - Max depth of 10 directories
#   - Filenames with tabs or newlines are not supported

if [[ $# -lt 2 ]]; then
    echo "Usage: generate-index.sh <directory-path> <section-name> [instruction-text]" >&2
    exit 1
fi

dir_path="$1"
section_name="$2"
instruction_text="${3:-}"

if [[ ! -d "${dir_path}" ]]; then
    echo "Error: directory not found: ${dir_path}" >&2
    exit 1
fi

# Temp files for processing
tmpfile=$(mktemp)
groups_file=$(mktemp)
trap 'rm -f "${tmpfile}" "${groups_file}"' EXIT

# Step 1: Find .md files, categorize by group (immediate subdir), sort
# Excludes hidden files/dirs (.*) and node_modules
find "${dir_path}" -maxdepth 10 \
    \( -name '.*' -o -name 'node_modules' \) -prune -o \
    -name '*.md' -print0 \
    | while IFS= read -r -d '' file; do
        rel="${file#"${dir_path}"/}"
        if [[ "${rel}" == */* ]]; then
            group="${rel%%/*}"
            file_in_group="${rel#*/}"
        else
            group="."
            file_in_group="${rel}"
        fi
        printf '%s\t%s\n' "${group}" "${file_in_group}"
    done \
    | LC_ALL=C sort > "${tmpfile}"

# Check if any files found
if [[ ! -s "${tmpfile}" ]]; then
    echo "Warning: no .md files found in ${dir_path}" >&2
    exit 0
fi

# Step 2: Aggregate files by group (tmpfile is sorted so groups are contiguous)
current_group=""
current_files=""

while IFS=$'\t' read -r group file_in_group; do
    if [[ "${group}" != "${current_group}" ]]; then
        if [[ -n "${current_group}" ]]; then
            printf '%s\t%s\n' "${current_group}" "${current_files}" >> "${groups_file}"
        fi
        current_group="${group}"
        current_files="${file_in_group}"
    else
        current_files="${current_files},${file_in_group}"
    fi
done < "${tmpfile}"

if [[ -n "${current_group}" ]]; then
    printf '%s\t%s\n' "${current_group}" "${current_files}" >> "${groups_file}"
fi

# Step 3: Build output line
output="[${section_name}]|root: ${dir_path}"
if [[ -n "${instruction_text}" ]]; then
    output="${output}|${instruction_text}"
fi

# "." group first (root-level files), then other groups alphabetically
dot_files=$(awk -F'\t' '$1 == "." {print $2}' "${groups_file}")
if [[ -n "${dot_files}" ]]; then
    output="${output}|.:{${dot_files}}"
fi

while IFS=$'\t' read -r group files; do
    output="${output}|${group}:{${files}}"
done < <(awk -F'\t' '$1 != "."' "${groups_file}" | LC_ALL=C sort)

echo "${output}"
