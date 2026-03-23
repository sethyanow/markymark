---
id: marky-fe7
title: 'fix(lefthook): add zig binary presence check to 07-zig-build hook'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

lefthook.yml 07-zig-build hook runs 'zig build -Doptimize=Debug' without checking if zig is installed. Missing zig yields a cryptic shell error. Other hooks (cargo-audit, gitleaks) already include presence checks with actionable messages. Add: 'command -v zig >/dev/null || { echo "zig is required: https://ziglang.org/download/"; exit 1; };' before the build command. Flagged by CodeRabbit in PR #36.
