---
id: marky-4g8
title: 'json_keys.zig: SIMD JSON key path extractor (simdjson-inspired)'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3.7
---



## Design

## Goal
Create zig/src/kernels/formats/json_keys.zig. SIMD brace/bracket/colon scanning for JSON key path extraction. Track nesting depth to build dot-separated key paths. Most complex format extractor.

## Effort Estimate
5-6 hours

## Success Criteria
- [ ] json_keys.zig compiles and exports marky_scan_json_keys
- [ ] Extracts key paths (e.g., "root.child.key") with byte offsets
- [ ] Tracks nesting depth via { } and [ ] counting
- [ ] Handles escaped quotes in string values
- [ ] Depth limit of 100 (returns -2 beyond)
- [ ] cd zig && zig build test passes

## Edge Cases
- Empty JSON: {} returns 0 keys
- Deeply nested JSON (>100 levels): cap depth, return -2
- Escaped quotes in strings: \" must not terminate string scanning
- Unicode escapes: \u0041 in strings — skip correctly
- Arrays: keys inside arrays get array index in path

## Anti-patterns
- NO building a full JSON parser (key path extraction only)
- NO unlimited depth tracking (stack overflow risk)
- NO assuming no nested arrays/objects

## Test Specifications
- test_json_flat_keys: catches basic key extraction failure
- test_json_nested_keys: catches incorrect depth tracking
- test_json_escaped_quotes: catches premature string termination
- test_json_depth_limit: catches stack overflow on deep nesting
- test_json_array_keys: catches missing array index in path
