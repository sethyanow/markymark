---
id: marky-pms
title: Remove dead tree-sitter-xml dependency
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-6gw
---



tree-sitter-xml 0.6 is declared in workspace and parser Cargo.toml but never imported in any .rs file. XML-in-markdown extraction uses custom stack tokenizer in extract.rs. Remove from both Cargo.toml files, run cargo build to verify, run xml_extraction tests to confirm custom parser still works.
