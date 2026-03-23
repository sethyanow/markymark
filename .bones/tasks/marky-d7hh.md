---
id: marky-d7hh
title: 'from_blob wiki link alias parity bug: text!=page should be text!=target'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

from_blob compares text != page (anchor-stripped target) for alias detection, but from_scan compares l.text != l.target (full target). For [[page#heading|page]], from_blob incorrectly sets alias=None and uses wrong end_byte formula. Source: cursor review.

## Design

## Goal

Fix alias detection parity between from_blob and from_scan for wiki links with heading anchors.

## Root Cause

from_blob.rs:414 compares \`text != page\` where \`page\` is the target with \`#heading\` stripped.
from_scan (mod.rs:626) compares \`l.text != l.target\` where \`l.target\` includes the anchor.

For \`[[page#heading|page]]\`:
- from_scan: \"page\" != \"page#heading\" → alias = Some(\"page\") (CORRECT)
- from_blob: \"page\" != \"page\" → alias = None (WRONG)

Cascade: wrong alias → wrong end_byte formula (line 535 uses \`target_len + 4\` instead of \`target_len + text_len + 5\`).

## Effort Estimate

2-3 hours (small, focused fix + regression tests)

## Implementation Checklist

- [ ] In from_blob.rs:414, change \`text != page\` to \`text != target\` (compare against full target BEFORE splitting on '#')
- [ ] Verify end_byte formula at line 535 now uses correct alias branch
- [ ] Add regression test: \`test_from_blob_wiki_link_with_heading_and_matching_alias\` for \`[[page#heading|page]]\`
- [ ] Add regression test: \`test_from_blob_wiki_link_with_heading_and_different_alias\` for \`[[page#heading|other]]\`
- [ ] Add \`[[Page#heading|Page]]\` case to the parity test at line 840 to prevent future regression
- [ ] Run \`cargo nextest -p markymark-index\` — all tests pass
- [ ] Run \`cargo clippy --workspace --all-targets\` — clean

## Success Criteria

- [ ] For \`[[page#heading|page]]\`, from_blob produces alias=Some(\"page\"), heading=Some(\"heading\"), target=\"page\" — matching from_scan
- [ ] For \`[[page#heading|other]]\`, from_blob produces alias=Some(\"other\") — matching from_scan
- [ ] For \`[[page]]\`, from_blob produces alias=None — no regression
- [ ] For \`[[page|alias]]\`, from_blob produces alias=Some(\"alias\") — no regression
- [ ] Parity test includes anchored-alias case and passes
- [ ] end_byte for anchored-alias wiki links matches from_scan's calculation
- [ ] \`cargo nextest -p markymark-index\` passes (all existing + new tests)
- [ ] \`cargo clippy --workspace --all-targets\` clean

## Key Considerations (SRE Review)

**Edge Case: [[page#heading]] (anchor, no alias)**
Current from_blob: text=\"page#heading\", page=\"page\" → text != page → alias=Some(\"page#heading\"). This is WRONG — no pipe separator means no alias.
Wait — need to verify what the blob actually stores for text and target. In the Zig engine, StoredLink has text and target fields. For \`[[page#heading]]\`:
- text = display text (extracted by md4c) = \"page#heading\" or \"page\"
- target = full link target = \"page#heading\"
Must verify md4c extraction renderer behavior for this case before implementing. The fix must not break this case.

**Edge Case: [[page#|alias]] (empty heading)**
Target = \"page#\", page = \"page\", heading = Some(\"\"). Text = \"alias\". Should detect alias correctly with the fix (\"alias\" != \"page#\").

**Edge Case: [[#heading|text]] (self-referencing heading link)**
Target = \"#heading\", page = \"\", heading = Some(\"heading\"). Text = \"text\". Should detect alias (\"text\" != \"#heading\").

**Verification: end_byte**
After fix, the alias branch at line 535 fires correctly, computing start_byte + target_len + text_len + 5. Verify target_len in the blob is the FULL target length (including anchor) since that's what the Zig blob serializer stores.

## Anti-patterns
- Do NOT change from_scan to match from_blob's (broken) behavior
- Do NOT strip anchor from target before comparison — compare full strings
- Do NOT modify blob format — fix is purely in Rust from_blob interpretation
