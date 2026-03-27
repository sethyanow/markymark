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

CONFIGS="--config=release"
case "$(uname -s)" in
  Darwin) CONFIGS="$CONFIGS --config=macos-lto" ;;
esac

echo "Building markymark (release + LTO)..."
bazel build $CONFIGS //markymark-cli:markymark

cp -f "$(bazel info bazel-bin $CONFIGS)/markymark-cli/markymark" "$INSTALL_DIR/markymark"
echo "installed: $INSTALL_DIR/markymark ($(${INSTALL_DIR}/markymark --version 2>/dev/null || echo 'version check skipped'))"
