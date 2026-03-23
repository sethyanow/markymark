---
id: marky-36t
title: 'Upgrade Cargo.toml deps: tree-sitter 0.26, tree-sitter-md 0.5, tree-sitter-json 0.24'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-pms]
parent: marky-6gw
---





Update workspace Cargo.toml: remove tree-sitter-markdown and tree-sitter-xml entries, add tree-sitter-md = "0.5", change tree-sitter = "0.26", change tree-sitter-json = "0.24". Update markymark-parser/Cargo.toml workspace refs. Run cargo check to see what breaks — this will produce compilation errors that subsequent tasks fix.
