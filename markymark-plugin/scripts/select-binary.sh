#!/usr/bin/env bash
# select-binary.sh — Execute the markymark binary.
#
# In the CI pre-packaged model, each per-platform plugin archive
# contains a single bin/markymark binary already built for the
# target platform. This script finds and executes it.
#
# If the binary is missing (e.g. marketplace git-clone install),
# the script auto-downloads the correct platform binary from
# GitHub Releases. If download fails, it shows manual instructions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${SCRIPT_DIR}/../bin"
REPO="sethyanow/markymark"

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

download_binary() {
    local target="$1"
    local dest="$2"
    local asset_name="markymark-${target}"
    local url="https://github.com/${REPO}/releases/latest/download/${asset_name}"

    echo "Downloading markymark for ${target}..." >&2

    mkdir -p "$(dirname "${dest}")"

    if curl -fsSL --retry 2 --retry-delay 1 -o "${dest}" "${url}"; then
        chmod +x "${dest}"
        echo "Downloaded successfully." >&2
        return 0
    else
        rm -f "${dest}"
        return 1
    fi
}

main() {
    local binary="${BIN_DIR}/markymark"

    if [[ ! -f "${binary}" ]]; then
        local target
        target="$(detect_target)"

        # Attempt auto-download from GitHub Releases
        if ! download_binary "${target}" "${binary}"; then
            echo "error: download failed and binary not found: ${binary}" >&2
            echo "hint: download markymark-plugin-${target}.tar.gz from GitHub Releases" >&2
            echo "      https://github.com/${REPO}/releases" >&2
            exit 1
        fi
    fi

    if [[ ! -x "${binary}" ]]; then
        chmod +x "${binary}"
    fi

    exec "${binary}" "$@"
}

main "$@"
