#!/usr/bin/env bash
# select-binary.sh — Platform detection and binary selection for markymark.
#
# Detects the host OS and CPU architecture, then executes the correct
# pre-built markymark binary from the plugin's bin/ directory.
#
# Supported platforms:
#   macOS  ARM64  → markymark-aarch64-apple-darwin
#   macOS  x86_64 → markymark-x86_64-apple-darwin
#   Linux  x86_64 → markymark-x86_64-unknown-linux-gnu
#   Linux  ARM64  → markymark-aarch64-unknown-linux-gnu
#   Windows x86_64 → markymark-x86_64-pc-windows-msvc.exe

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
                *)             echo "error: unsupported macOS architecture: ${arch}" >&2; exit 1 ;;
            esac
            ;;
        Linux)
            case "${arch}" in
                aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
                x86_64)        target="x86_64-unknown-linux-gnu" ;;
                *)             echo "error: unsupported Linux architecture: ${arch}" >&2; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*|Windows_NT)
            case "${arch}" in
                x86_64|AMD64) target="x86_64-pc-windows-msvc" ;;
                *)            echo "error: unsupported Windows architecture: ${arch}" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "error: unsupported operating system: ${os}" >&2
            exit 1
            ;;
    esac

    echo "${target}"
}

main() {
    local target binary

    target="$(detect_target)"
    binary="${BIN_DIR}/markymark-${target}"

    # Windows binaries have .exe extension
    if [[ "${target}" == *windows* ]]; then
        binary="${binary}.exe"
    fi

    if [[ ! -f "${binary}" ]]; then
        echo "error: binary not found: ${binary}" >&2
        echo "hint: run the release build or download from GitHub Releases" >&2
        exit 1
    fi

    if [[ ! -x "${binary}" ]]; then
        chmod +x "${binary}"
    fi

    exec "${binary}" "$@"
}

main "$@"
