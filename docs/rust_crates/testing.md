# Testing - insta & proptest

<agent>
<goal>Write effective tests using snapshot testing (insta) and property-based testing (proptest).</goal>
<when_to_use>When testing parsers, formatters, or any code with complex outputs (insta) or when testing invariants across many inputs (proptest).</when_to_use>
<contains>insta snapshots, assert_snapshot!, proptest strategies, shrinking, test patterns</contains>
<see_also>tree-sitter.md, error-handling.md</see_also>
</agent>

**TL;DR:** Use `insta` for snapshot testing (parser output, formatted text, AST). Use `proptest` for property-based testing (invariants, roundtrips, edge cases). They complement each other well.

**Checklist:**
- [ ] Use `insta` for complex output comparison (AST, formatted output)
- [ ] Use `proptest` for invariant testing ("parse never panics", "roundtrip works")
- [ ] Run `cargo insta review` after test failures to update snapshots
- [ ] Write custom `proptest` strategies for domain-specific inputs

---

## insta - Snapshot Testing

### Setup

```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml", "json"] }
```

### Basic Snapshots

```rust
use insta::assert_snapshot;

#[test]
fn test_heading_parsing() {
    let input = "# Hello World";
    let output = parse_heading(input);

    // First run: creates snapshot file
    // Subsequent runs: compares against snapshot
    assert_snapshot!(output);
}

#[test]
fn test_with_name() {
    let output = format_document(doc);

    // Named snapshot for clarity
    assert_snapshot!("formatted_basic_doc", output);
}
```

### Debug Snapshots (Structs)

```rust
use insta::assert_debug_snapshot;

#[derive(Debug)]
struct Heading {
    level: u8,
    text: String,
    slug: String,
}

#[test]
fn test_heading_struct() {
    let heading = parse_heading("## Introduction");

    // Snapshots the Debug output
    assert_debug_snapshot!(heading);
}
```

Snapshot file (`snapshots/test__test_heading_struct.snap`):
```
---
source: src/parser.rs
expression: heading
---
Heading {
    level: 2,
    text: "Introduction",
    slug: "introduction",
}
```

### YAML/JSON Snapshots

```rust
use insta::{assert_yaml_snapshot, assert_json_snapshot};
use serde::Serialize;

#[derive(Serialize)]
struct Document {
    title: String,
    headings: Vec<Heading>,
}

#[test]
fn test_document_yaml() {
    let doc = parse_document("# Title\n## Section");
    assert_yaml_snapshot!(doc);
}

#[test]
fn test_document_json() {
    let doc = parse_document("# Title");
    assert_json_snapshot!(doc);
}
```

### Inline Snapshots

```rust
use insta::assert_snapshot;

#[test]
fn test_inline() {
    let output = slugify("Hello World!");

    // Snapshot stored inline in test file
    assert_snapshot!(output, @"hello-world");
}

// After `cargo insta review`, the @"..." part is auto-updated
```

### Redactions (Filtering Unstable Values)

```rust
use insta::{assert_yaml_snapshot, with_settings};

#[test]
fn test_with_timestamp() {
    let result = process_document();

    with_settings!({
        // Redact fields that change between runs
        redactions => {
            "[].timestamp" => "[timestamp]",
            "[].id" => "[id]",
        }
    }, {
        assert_yaml_snapshot!(result);
    });
}
```

### Snapshot Settings

```rust
use insta::{assert_snapshot, with_settings};

#[test]
fn test_with_settings() {
    with_settings!({
        // Change snapshot path
        snapshot_path => "../snapshots",
        // Add description
        description => "Tests heading parsing",
        // Omit expression from snapshot
        omit_expression => true,
    }, {
        assert_snapshot!(parse("# Test"));
    });
}
```

### Workflow

```bash
# Run tests (new snapshots marked as pending)
cargo test

# Review pending snapshots interactively
cargo insta review

# Accept all pending snapshots
cargo insta accept

# Reject all pending snapshots
cargo insta reject
```

---

## proptest - Property-Based Testing

### Setup

```toml
[dev-dependencies]
proptest = "1"
```

### Basic Properties

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_slugify_doesnt_panic(input in ".*") {
        // Property: slugify never panics
        let _ = slugify(&input);
    }

    #[test]
    fn test_slugify_no_spaces(input in "[a-zA-Z ]+") {
        // Property: output never contains spaces
        let slug = slugify(&input);
        prop_assert!(!slug.contains(' '));
    }

    #[test]
    fn test_slugify_lowercase(input in "[a-zA-Z]+") {
        // Property: output is lowercase
        let slug = slugify(&input);
        prop_assert_eq!(slug, slug.to_lowercase());
    }
}
```

### Custom Strategies

```rust
use proptest::prelude::*;

// Strategy for valid markdown headings
fn heading_strategy() -> impl Strategy<Value = String> {
    (1..=6usize, "[a-zA-Z0-9 ]+")
        .prop_map(|(level, text)| {
            format!("{} {}", "#".repeat(level), text)
        })
}

proptest! {
    #[test]
    fn test_parse_heading(input in heading_strategy()) {
        let result = parse_heading(&input);
        prop_assert!(result.is_ok());
    }
}

// Strategy for wiki links
fn wiki_link_strategy() -> impl Strategy<Value = String> {
    ("[a-zA-Z0-9_-]+", prop::option::of("[a-zA-Z0-9 ]+"))
        .prop_map(|(target, alias)| {
            match alias {
                Some(a) => format!("[[{}|{}]]", target, a),
                None => format!("[[{}]]", target),
            }
        })
}
```

### Struct Strategies with Arbitrary

```rust
use proptest::prelude::*;
use proptest_derive::Arbitrary;

#[derive(Debug, Clone, Arbitrary)]
struct TestHeading {
    #[proptest(strategy = "1..=6u8")]
    level: u8,
    #[proptest(regex = "[a-zA-Z][a-zA-Z0-9 ]{0,50}")]
    text: String,
}

proptest! {
    #[test]
    fn test_heading_roundtrip(heading in any::<TestHeading>()) {
        let markdown = format!("{} {}", "#".repeat(heading.level as usize), heading.text);
        let parsed = parse_heading(&markdown).unwrap();
        prop_assert_eq!(parsed.level, heading.level);
    }
}
```

### Roundtrip Testing

```rust
proptest! {
    #[test]
    fn test_parse_format_roundtrip(
        level in 1..=6u8,
        text in "[a-zA-Z][a-zA-Z0-9 ]{0,100}"
    ) {
        let original = format!("{} {}", "#".repeat(level as usize), text);

        // Parse
        let parsed = parse_heading(&original)?;

        // Format back
        let formatted = format_heading(&parsed);

        // Should match (modulo whitespace normalization)
        prop_assert_eq!(
            original.trim(),
            formatted.trim()
        );
    }
}

// Roundtrip for documents
proptest! {
    #[test]
    fn test_document_roundtrip(doc in document_strategy()) {
        let formatted = format_document(&doc);
        let reparsed = parse_document(&formatted)?;

        // Structural equality
        prop_assert_eq!(doc.headings.len(), reparsed.headings.len());
        for (a, b) in doc.headings.iter().zip(reparsed.headings.iter()) {
            prop_assert_eq!(a.level, b.level);
            prop_assert_eq!(a.text.trim(), b.text.trim());
        }
    }
}
```

### Testing Invariants

```rust
proptest! {
    // Connection graph invariants
    #[test]
    fn test_graph_invariants(
        docs in prop::collection::vec(document_strategy(), 1..20)
    ) {
        let graph = build_connection_graph(&docs);

        // Invariant: every node has valid index
        for node in graph.node_indices() {
            prop_assert!(graph.contains_node(node));
        }

        // Invariant: every edge connects existing nodes
        for edge in graph.edge_references() {
            prop_assert!(graph.contains_node(edge.source()));
            prop_assert!(graph.contains_node(edge.target()));
        }

        // Invariant: backref count matches incoming edge count
        for node in graph.node_indices() {
            let incoming = graph.edges_directed(node, Incoming).count();
            let backrefs = graph.backrefs.get(&node).map_or(0, |v| v.len());
            prop_assert_eq!(incoming, backrefs);
        }
    }
}
```

### Shrinking

proptest automatically shrinks failing inputs to minimal cases:

```rust
proptest! {
    #[test]
    fn test_that_might_fail(input in ".{1,1000}") {
        // If this fails on a 500-char string,
        // proptest will shrink to find minimal failing case
        let result = process(&input);
        prop_assert!(result.len() < 100);
    }
}

// Output might show:
// proptest: Shrinking input "aaa...long string..."
// proptest: Minimal failing case: "a"
```

### Configuration

```rust
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,           // Run 1000 cases (default: 256)
        max_shrink_iters: 100, // Limit shrinking iterations
        ..ProptestConfig::default()
    })]

    #[test]
    fn expensive_test(input in complex_strategy()) {
        // ...
    }
}
```

---

## Combined Patterns

### Snapshot Testing with proptest

```rust
use insta::assert_debug_snapshot;
use proptest::prelude::*;

// Use proptest to find edge cases, then snapshot them
#[test]
fn snapshot_edge_cases() {
    // Known edge cases found by proptest
    let cases = vec![
        "",
        "#",
        "# ",
        "######",
        "####### Too Many",
        "# Title\n## Nested",
    ];

    for (i, input) in cases.iter().enumerate() {
        let result = parse(input);
        assert_debug_snapshot!(format!("edge_case_{}", i), result);
    }
}
```

### Golden File Testing (insta pattern)

```rust
use std::fs;
use insta::assert_snapshot;

#[test]
fn test_all_fixtures() {
    let fixtures_dir = "tests/fixtures/";

    for entry in fs::read_dir(fixtures_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "md") {
            let input = fs::read_to_string(&path).unwrap();
            let output = process(&input);

            let name = path.file_stem().unwrap().to_str().unwrap();
            assert_snapshot!(name, output);
        }
    }
}
```

### Fuzzing-Lite with proptest

```rust
proptest! {
    // Fuzz-like testing: random inputs shouldn't crash
    #[test]
    fn fuzz_parser(input in prop::collection::vec(any::<u8>(), 0..10000)) {
        let input_str = String::from_utf8_lossy(&input);
        // Should not panic
        let _ = parse(&input_str);
    }

    #[test]
    fn fuzz_incremental_parser(
        initial in ".{0,5000}",
        edits in prop::collection::vec(
            (0..100usize, 0..100usize, ".{0,100}"),
            0..20
        )
    ) {
        let mut doc = Document::new(&initial);

        for (start, len, new_text) in edits {
            let end = (start + len).min(doc.len());
            let start = start.min(doc.len());
            // Apply edit - should not panic
            doc.apply_edit(start, end, &new_text);
        }
    }
}
```

---

## Pitfalls

### Snapshot Pollution

<pitfall>
**Problem:** Snapshot files accumulate for renamed/deleted tests.

```
snapshots/
  old_test_name.snap     # Orphaned
  renamed_test.snap      # Current
```

**Solution:** Clean orphaned snapshots:

```bash
# List orphaned snapshots
cargo insta test --delete-unreferenced-snapshots

# Or manually check snapshots/ directory
```
</pitfall>

### Non-Deterministic Snapshots

<pitfall>
**Problem:** Snapshots differ between runs due to timestamps, random IDs, etc.

```rust
// BAD: Contains timestamp
assert_snapshot!(result_with_timestamp);
```

**Solution:** Use redactions or filter before snapshot:

```rust
// GOOD: Redact variable fields
with_settings!({
    redactions => {
        ".timestamp" => "[timestamp]",
        ".id" => "[id]",
    }
}, {
    assert_yaml_snapshot!(result);
});

// Or filter before
let filtered = result.with_timestamp(None);
assert_snapshot!(filtered);
```
</pitfall>

### proptest Slowness

<pitfall>
**Problem:** proptest runs too slow with complex strategies.

```rust
// BAD: Very slow
proptest! {
    #[test]
    fn slow_test(input in ".{0,100000}") {
        expensive_operation(&input);
    }
}
```

**Solution:** Limit size, reduce cases, or use `#[ignore]`:

```rust
// GOOD: Reasonable limits
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn faster_test(input in ".{0,1000}") {
        expensive_operation(&input);
    }
}

// Or mark as slow test
#[test]
#[ignore] // Run with `cargo test -- --ignored`
fn slow_comprehensive_test() {
    proptest!(|(input in ".{0,100000}")| {
        expensive_operation(&input);
    });
}
```
</pitfall>

### Strategy Complexity

<pitfall>
**Problem:** Complex strategies generate invalid inputs.

```rust
// BAD: May generate invalid markdown
fn bad_strategy() -> impl Strategy<Value = String> {
    ".*"  // Includes binary, control chars, etc.
}
```

**Solution:** Constrain to valid domain:

```rust
// GOOD: Generate valid markdown
fn markdown_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            heading_strategy(),
            paragraph_strategy(),
            list_strategy(),
        ],
        1..10
    ).prop_map(|parts| parts.join("\n\n"))
}
```
</pitfall>

---

## Related

- Parser testing: `tree-sitter.md`
- Error testing: `error-handling.md`
- insta docs: https://insta.rs/
- proptest docs: https://proptest-rs.github.io/proptest/
- proptest book: https://proptest-rs.github.io/proptest/proptest/index.html
