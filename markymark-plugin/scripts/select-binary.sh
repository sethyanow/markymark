#!/usr/bin/env bash
# select-binary.sh — Execute the bundled markymark binary.
#
# In the CI pre-packaged model, each per-platform plugin archive
# contains a single bin/markymark binary already built for the
# target platform. This script simply finds and executes it.
#
# If the binary is missing (e.g. dev checkout without a build),
# the error message includes the platform-specific archive name
# so the user can download the correct one from GitHub Releases.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${SCRIPT_DIR}/../bin"

detect_target() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Darwin)
            case "${arch}" in
                arm64|aarch64) target="aarch64-apple-darwin" ;;
                x86_64)        target="x86_64-apple-darwin" ;;
                *)             target="unknown-${os}-${arch}" ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
                x86_64)        target="x86_64-unknown-linux-gnu" ;;
                *)             target="unknown-${os}-${arch}" ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*|Windows_NT)
            case "${arch}" in
                x86_64|AMD64) target="x86_64-pc-windows-msvc" ;;
                *)            target="unknown-${os}-${arch}" ;;
            esac
            ;;
        *)
            target="unknown-${os}-${arch}"
            ;;
    esac

    echo "${target}"
}

main() {
    local binary="${BIN_DIR}/markymark"

    if [[ ! -f "${binary}" ]]; then
        local target
        target="$(detect_target)"
        echo "error: binary not found: ${binary}" >&2
        echo "hint: download markymark-plugin-${target}.tar.gz from GitHub Releases" >&2
        echo "      https://github.com/sethyanow/markymark/releases" >&2
        exit 1
    fi

    if [[ ! -x "${binary}" ]]; then
        chmod +x "${binary}"
    fi

    exec "${binary}" "$@"
}

main "$@"
