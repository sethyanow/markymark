---
id: marky-5fcg
title: Enhance ownership transfer comment in document.zig parseAll
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Code review nitpick: existing comment at lines 282-283 explains ownership transfer but could reference specific variables for clarity.

## Design

## Goal
Enhance the ownership transfer comment at document.zig:282-286 to explicitly reference the variables involved in the string data ownership transfer from extraction to stored lists.

## Why (Gap Analysis)
The existing comment (lines 282-283) correctly states that only slice containers are freed, not the string contents. However, it does not name the specific variables involved, which could lead a future refactor to mistakenly free the moved strings.

## Effort Estimate
15-30 minutes

## Success Criteria
- [ ] Comment at lines 282-286 explicitly names: extraction.headings, extraction.links, stored_headings_list, stored_links_list, extraction_renderer
- [ ] Comment explains that h.text and l.text/l.target string pointers were moved into stored lists by the loops above
- [ ] Comment warns against freeing the string contents here
- [ ] No code changes — comment only
- [ ] Zig build succeeds (no syntax errors in comment)

## Implementation Checklist
- [ ] Open zig/src/engine/document.zig
- [ ] Replace the two-line comment at lines 282-283 with an expanded version
- [ ] New comment should be 4-6 lines, referencing specific variable names
- [ ] Run zig build to verify no syntax errors
- [ ] Verify visually that the comment reads clearly

## Exact Change

Replace lines 282-285:
```zig
    // Headings and links are now owned by stored lists.
    // Free extraction containers only (not the strings inside, since they're transferred).
    allocator.free(extraction.headings);
    allocator.free(extraction.links);
```

With:
```zig
    // OWNERSHIP: The string data (h.text, l.text, l.target) from extraction_renderer's
    // ExtractedHeading/ExtractedLink arrays has been moved into stored_headings_list and
    // stored_links_list by the loops above (steps 4-5). Only the slice containers
    // (extraction.headings, extraction.links) are freed here — NOT the string contents.
    // Do not add allocator.free(h.text) or similar; the strings are now owned by stored lists.
    allocator.free(extraction.headings);
    allocator.free(extraction.links);
```

## Key Considerations (SRE Review)

**Edge Case: errdefer Paths**
The errdefer blocks (lines 204-209) call freeStoredHeadingsList and freeStoredLinksList which have their own ownership rules. The comment added here applies only to the success path (after step 5). The errdefer handlers already have inline comments.

**Test Meaningfulness**
No test needed — this is a comment-only change. The existing memory leak tests (test_create_destroy_no_leaks, test_update_100_times_no_leaks) validate correct ownership.

## Anti-patterns
- Do NOT change any code — comment only
- Do NOT add redundant comments elsewhere (the errdefer blocks already have notes)
- Keep the comment concise (4-6 lines max)
