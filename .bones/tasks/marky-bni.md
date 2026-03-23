---
id: marky-bni
title: Migrate markdown parser to tree-sitter-md MarkdownParser wrapper
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-36t]
parent: marky-6gw
---




The core migration task. Replace tree-sitter-markdown usage with tree-sitter-md MarkdownParser. Changes needed: (1) lib.rs: replace TSParser + tree_sitter_markdown::language() with MarkdownParser::default(), update parse() to return MarkdownTree, adapt root_node() to block_tree().root_node(). (2) ast.rs: update tree traversal to work with MarkdownTree block_tree(). (3) types.rs: update node kind strings — tight_list/loose_list → list, verify all other kinds. (4) Ensure Parser public API (parse method signature, Ast return type) is preserved for downstream consumers. Write tests first to verify current behavior, then migrate.
