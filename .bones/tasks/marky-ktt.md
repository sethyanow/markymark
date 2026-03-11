---
id: marky-ktt
title: Audit unsafe transmute usage in DocumentIndex from marky-5yt migration
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Context

PR #21 (feature/mark-next) has 20 Semgrep security findings flagging unsafe usage in markymark-index/src/document.rs. These are part of the marky-5yt self_cell migration that replaced ptr::read + mem::forget with multiple mem::transmute calls.

## Security Assessment

**Risk Level:** MEDIUM (requires architecture review)

The marky-5yt task (closed as complete) migrated DocumentIndex from:
- **OLD**: ptr::read() + mem::forget() to transfer arena ownership
- **NEW**: 6 mem::transmute() calls to extend lifetimes from AST borrows to 'static

## Unsafe Blocks to Audit (7 total)

### Transmute Calls (6)
1. Line ~217: ast.root_elements() - extends &[Element<'a>] to &[Element<'static>]
2. Line ~238: ast.extract_block_ids() - extends Vec<BlockId<'a>> to Vec<BlockId<'static>>
3. Line ~256: ast.extract_wiki_links() - extends Vec<WikiLink<'a>> to Vec<WikiLink<'static>>
4. Line ~267: ast.extract_tags() - extends Vec<Tag<'a>> to Vec<Tag<'static>>
5. Line ~278: ast.extract_markdown_links() - extends Vec<MarkdownLink<'a>> to Vec<MarkdownLink<'static>>
6. Line ~299: ast.extract_xml_tags() - extends Vec<XmlTag<'a>> to Vec<XmlTag<'static>>

### Raw Pointer Dereference (1)
7. Line 221: unsafe { (*arena_ptr).bump() } - arena access with proper SAFETY comment

All unsafe blocks have SAFETY comments. Tests pass (606 green). Miri validation exists.

## Concerns

1. **Design Mismatch**: marky-5yt was supposed to use self_cell to ELIMINATE unsafe, not add more
2. **Multiple Transmutes**: 6 lifetime extensions via transmute is fragile
3. **Lifetime Soundness**: Correctness depends on ownership transfer semantics working as designed
4. **Alternative Approaches**: Should evaluate self_cell or ouroboros to eliminate transmute

## Recommended Actions

### P0 - Immediate
- [ ] Verify marky-5yt self_cell migration is actually complete (task shows closed but uses transmute)
- [ ] Run cargo +nightly miri test -p markymark-index focusing on arena transfer
- [ ] Audit each SAFETY comment for completeness and correctness
- [ ] Review unsafe blocks with domain expert familiar with self_cell patterns

### P1 - Short Term
- [ ] Compare implementation with self_cell crate best practices
- [ ] Check if current approach matches intended self_cell pattern
- [ ] Add comprehensive unsafe documentation (invariants, lifetime extension rationale)
- [ ] Expand test coverage for arena transfer edge cases

### P2 - Medium Term
- [ ] Evaluate if self_cell can eliminate transmute need
- [ ] Consider ouroboros crate as safer alternative
- [ ] Assess if 'static lifetime is necessary for tower-lsp Send requirements
- [ ] Add stress tests for DocumentIndex with concurrent LSP operations

## Additional Context

- PR #21 has 10 other code review comments on parser/LSP files (not yet triaged)
- All quality gates passing (tests, clippy, Miri arena validation)
- Lifetime boundary hardening added compile_fail regression tests
- Branch is in RED ZONE: 12881 lines, 82 commits, 49 new files

## Files

- Triage analysis: pr21-triage-detailed.md
- Raw comments: pr21-comments.json
- Structured triage: pr21-triage.json

## Links

- PR #21: https://github.com/sethyanow/markymark/pull/21
- Parent task: marky-5yt (closed)
- Security findings: GitHub Advanced Security Semgrep alerts
