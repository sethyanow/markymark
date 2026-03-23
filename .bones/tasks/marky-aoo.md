---
id: marky-aoo
title: Full test suite verification and node kind audit
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-bni, marky-j7h]
parent: marky-6gw
---




Final verification task. (1) Run full cargo test — all 391+ tests must pass. (2) Audit every node.kind() match in types.rs and ast.rs against tree-sitter-md grammar node-types.json. (3) Run cargo clippy --workspace --all-targets — must be clean. (4) Verify cargo tree shows single tree-sitter version (no duplicates). (5) Run pre-commit hooks (lefthook). (6) Commit and push.
