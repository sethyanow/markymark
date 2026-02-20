# Golden Blob Provenance

**File:** `golden_v1.blob`
**Blob version:** 1
**Magic:** `0x4D4B5343` ("MKSC")
**Generated:** 2026-02-20
**Generator:** `gen_golden_blob` binary (removed after generation)

## Generating Markdown Input

```markdown
# Title One

## Section A

## Section A

[[Simple Link]]
[[Page Name|Display Text]]
[Click here](https://example.com)
[Anchored](doc.md#section)
tags: #alpha #beta #gamma
block one ^id-one
block two ^id-two
```

## Expected Counts

| Section     | Count |
|-------------|-------|
| headings    | 3     |
| links       | 4 (2 wiki + 2 markdown) |
| tags        | 3     |
| block IDs   | 2     |

## Regenerating

If `BLOB_VERSION` increments or the Zig blob format changes, regenerate with:

```bash
# Restore generator binary from git history, or re-create it, then:
cargo run -p markymark-index --bin gen_golden_blob --features zig-kernels
# Commit the new golden_v1.blob (or golden_v2.blob for a version bump)
# Update this file with the new generation date
```

When regenerating for a version bump, add a new `golden_v2.blob` alongside
the old file and update `test_golden_blob_roundtrip` to use the new file.
The old file can be removed once no longer needed for bisect.
