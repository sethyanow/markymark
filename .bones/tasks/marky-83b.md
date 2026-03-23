---
id: marky-83b
title: Aho-Corasick automaton construction (pattern compilation, goto/failure)
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.2
---




## Design

## Goal
Build the Aho-Corasick automaton for markdown pattern matching. Compile pattern set (newline+#, [, [[, #tag, ^, triple backtick, triple tilde) into a trie with goto function and KMP-style failure function. Automaton must be constructible at comptime or lazily initialized (no per-call allocation).

## Effort Estimate
5-6 hours

## Success Criteria
- [ ] Pattern trie constructed for all 7 markdown pattern prefixes
- [ ] Goto function handles all byte transitions from each state
- [ ] Failure function computes proper suffix links for overlapping patterns
- [ ] Automaton is comptime or lazy-static (no allocation per call)
- [ ] Unit tests verify pattern matching correctness on known inputs
- [ ] Handles overlapping patterns: [[ contains [ as prefix

## Edge Cases
- Overlapping patterns: [[ starts with [ — automaton must distinguish via longest match
- Pattern starting at byte 0 vs after newline: # headings only at line start
- All patterns absent: automaton reaches accept state 0 times, returns cleanly

## Anti-patterns
- NO building automaton at each call (must be static/comptime)
- NO O(alphabet * states) memory for goto (use sparse transitions)
- NO assuming ASCII-only input (patterns are ASCII but text is UTF-8)

## Test Specifications
- test_automaton_finds_heading: catches missing # pattern in trie
- test_automaton_finds_wiki_link: catches [[ not distinguished from [
- test_automaton_overlapping_prefix: catches longest-match failure
- test_automaton_no_matches: catches false positive on random text
