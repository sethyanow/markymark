---
id: marky-lkj.3
title: Implement TOML parser (tree-sitter-toml-ng)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Implement TOML parser using tree-sitter-toml-ng 0.7.0, following the same CST walker pattern as json.rs and yaml.rs. Add tree-sitter-toml-ng dependency, create structured/toml.rs, wire dispatch in mod.rs. Must handle: dotted keys (a.b.c), inline tables, arrays of tables, all value types. Same StructuredAst output interface.
