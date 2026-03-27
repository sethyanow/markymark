#!/bin/bash
# Wrapper for Homebrew clang that strips -plugin-opt args unsupported by ld64.lld.
# Used by rustc -Clinker-plugin-lto on macOS for cross-language ThinLTO.
# ld64.lld handles -flto=thin natively; the -plugin-opt flags are redundant.
args=()
for arg in "$@"; do
  case "$arg" in
    -Wl,-plugin-opt=*) ;;
    *) args+=("$arg") ;;
  esac
done
exec /opt/homebrew/opt/llvm/bin/clang "${args[@]}"
