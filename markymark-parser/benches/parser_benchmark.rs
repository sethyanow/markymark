use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use markymark_parser::Parser;

fn create_basic_markdown() -> &'static str {
    r#"
# Main Heading

This is a paragraph with some **bold** and *italic* text.

## Section 1

- List item 1
- List item 2
- List item 3

### Subsection

1. Numbered item
2. Another item
3. Final item

## Section 2

Here's a [link](https://example.com) and some `inline code`.

```rust
fn main() {
    println!("Hello, world!");
}
```

> A blockquote
> with multiple lines

## Links and References

This references [[another-note]] and has a wiki link.

[[note-with-alias|Display Text]]

## Tags

#tag1 #tag2 #tag3

---

Final paragraph.
"#
}

fn create_large_markdown() -> &'static str {
    let mut content = String::new();

    for i in 0..50 {
        content.push_str(&format!("# Section {}\n\n", i));

        for j in 0..10 {
            content.push_str(&format!("## Subsection {}-{}\n\n", i, j));
            content.push_str(&format!(
                "This is paragraph {} with **bold** and *italic* text.\n\n",
                j
            ));
            content.push_str("- List item\n");
            content.push_str("- Another item\n");
            content.push_str("- Third item\n\n");
            content.push_str(&format!("[[link-{}]] #tag-{}\n\n", j, j));
        }

        content.push_str("---\n\n");
    }

    Box::leak(content.into_boxed_str())
}

fn create_obsidian_markdown() -> &'static str {
    r#"
---
title: Obsidian Note
date: 2024-01-15
tags: [obsidian, markdown, example]
---

# Daily Note

## Tasks

- [x] Completed task
- [ ] Incomplete task
- [ ] Another task #urgent

## Notes

This note links to [[another-note]] and references ^block-id.

![[embedded-note]]

## Callouts

> [!note]
> This is a note callout
> with multiple lines

> [!warning] Important
> This is a warning with a title

## Query

```dataview
LIST
FROM #tag1
WHERE completed = false
```

## Properties

property1:: value1
property2:: [[linked-value]]

## Tags

#daily #note #obsidian #example

## Backlinks

This will appear in backlinks to [[other-notes]].
"#
}

fn bench_parser_basic(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_basic");

    let markdown = create_basic_markdown();
    group.throughput(Throughput::Bytes(markdown.len() as u64));

    group.bench_function("parse_basic", |b| {
        b.iter(|| {
            let mut parser = Parser::new().unwrap();
            black_box(parser.parse(black_box(markdown)).unwrap())
        })
    });

    group.finish();
}

fn bench_parser_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_large");

    let markdown = create_large_markdown();
    group.throughput(Throughput::Bytes(markdown.len() as u64));

    group.bench_function("parse_large", |b| {
        b.iter(|| {
            let mut parser = Parser::new().unwrap();
            black_box(parser.parse(black_box(markdown)).unwrap())
        })
    });

    group.finish();
}

fn bench_parser_obsidian(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_obsidian");

    let markdown = create_obsidian_markdown();
    group.throughput(Throughput::Bytes(markdown.len() as u64));

    group.bench_function("parse_obsidian", |b| {
        b.iter(|| {
            let mut parser = Parser::new().unwrap();
            black_box(parser.parse(black_box(markdown)).unwrap())
        })
    });

    group.finish();
}

fn bench_parser_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_reuse");

    let markdown = create_basic_markdown();
    group.throughput(Throughput::Bytes(markdown.len() as u64));

    group.bench_function("parse_with_reused_parser", |b| {
        let mut parser = Parser::new().unwrap();
        b.iter(|| black_box(parser.parse(black_box(markdown)).unwrap()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parser_basic,
    bench_parser_large,
    bench_parser_obsidian,
    bench_parser_reuse
);
criterion_main!(benches);
