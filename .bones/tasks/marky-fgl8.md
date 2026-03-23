---
id: marky-fgl8
title: Split extract.rs into submodules
status: open
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-ix3
---


## Goal
Split markymark-parser/src/extract.rs (863 lines) into submodules before adding code span extraction. This is a prerequisite for all marky-ix3 work.

## Implementation

1. Create markymark-parser/src/extract/ directory
2. Move extract.rs to extract/mod.rs
3. Extract into submodules following natural groupings:
   - extract/links.rs: extract_wiki_links, extract_markdown_links, extract_link_definitions, extract_embeds
   - extract/tags.rs: extract_tags, extract_xml_tags, collect_fenced_code_ranges
   - extract/blocks.rs: extract_block_ids, extract_block_refs, extract_callouts, extract_query_blocks
   - extract/frontmatter.rs: extract_frontmatter, extract_page_properties
   - extract/tasks.rs: extract_tasks
   - extract/mod.rs: re-exports, shared helpers, Extractor trait/types
4. Each step: move one module, run tests, commit
5. No behavior changes — pure mechanical refactor

## Success Criteria
- [ ] extract.rs split into 5+ submodules
- [ ] No single file exceeds 500 lines
- [ ] All existing tests pass unchanged
- [ ] cargo clippy clean
- [ ] Pre-commit hooks pass
- [ ] Public API unchanged (re-exports in mod.rs)
