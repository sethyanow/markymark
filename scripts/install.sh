#!/bin/bash
set -euo pipefail

# Install markymark to ~/.local/bin (or INSTALL_DIR).
# Builds with Bazel cross-language ThinLTO for optimized release binary.
#
# Prerequisites:
#   - Bazel (or Bazelisk): https://bazel.build/install
#   - macOS only: brew install llvm

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"

CONFIGS=(--config=release)
case "$(uname -s)" in
  Darwin)
    CONFIGS+=(--config=macos-lto)
    # Install clang-lto-wrapper alongside Homebrew LLVM if not already present
    LLVM_BIN="$(brew --prefix llvm)/bin"
    if [[ ! -x "${LLVM_BIN}/clang-lto-wrapper" ]]; then
      SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
      cp "${SCRIPT_DIR}/../tools/clang-lto-wrapper.sh" "${LLVM_BIN}/clang-lto-wrapper"
      chmod +x "${LLVM_BIN}/clang-lto-wrapper"
    fi
    ;;
esac

echo "Building markymark (release + LTO)..."
bazel build "${CONFIGS[@]}" //markymark-cli:markymark

cp -f "$(bazel info bazel-bin "${CONFIGS[@]}")/markymark-cli/markymark" "$INSTALL_DIR/markymark"
echo "installed: ${INSTALL_DIR}/markymark ($("${INSTALL_DIR}/markymark" --version 2>/dev/null || echo 'version check skipped'))"
