# tree-sitter - Incremental Parsing

<agent>
<goal>Parse markdown and other languages incrementally with tree-sitter bindings.</goal>
<when_to_use>When you need fast, incremental parsing with syntax tree access.</when_to_use>
<contains>Parser setup, tree-sitter-markdown, node traversal, queries, incremental updates, edit application</contains>
<see_also>tower-lsp.md, bumpalo.md</see_also>
</agent>

**TL;DR:** tree-sitter provides incremental parsing. Create a Parser with a Language, parse text to get a Tree, traverse Nodes. On edits, call `edit()` then re-parse for O(edit size) updates.

**Checklist:**
- [ ] Create `Parser` and set language
- [ ] Parse text with `parser.parse(text, old_tree)`
- [ ] Traverse tree with `root_node()`, `child()`, `children()`
- [ ] Use queries for pattern matching
- [ ] Call `tree.edit()` before re-parsing on changes

---

## Setup

### Cargo.toml

```toml
[dependencies]
tree-sitter = "0.22"
tree-sitter-markdown = { git = "https://github.com/tree-sitter-grammars/tree-sitter-markdown" }

[build-dependencies]
cc = "1"
```

### Basic Parsing

```rust
use tree_sitter::{Parser, Language, Tree, Node};
use tree_sitter_markdown;

fn create_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_markdown::language())
        .expect("Error loading markdown grammar");
    parser
}

fn parse_markdown(parser: &mut Parser, source: &str) -> Option<Tree> {
    parser.parse(source, None)
}

fn main() {
    let mut parser = create_parser();
    let source = "# Hello\n\nSome text with a [[wiki link]].";

    if let Some(tree) = parse_markdown(&mut parser, source) {
        let root = tree.root_node();
        println!("Root: {} [{}-{}]", root.kind(), root.start_byte(), root.end_byte());
        print_tree(&root, source, 0);
    }
}

fn print_tree(node: &Node, source: &str, indent: usize) {
    let prefix = "  ".repeat(indent);
    println!(
        "{}{} [{}-{}] {:?}",
        prefix,
        node.kind(),
        node.start_byte(),
        node.end_byte(),
        node.utf8_text(source.as_bytes()).ok()
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(&child, source, indent + 1);
    }
}
```

---

## Patterns

### tree-sitter-markdown Node Types

tree-sitter-markdown has two parsers: block and inline. The block parser handles document structure:

```
document
├── section
│   ├── atx_heading
│   │   ├── atx_h1_marker (#)
│   │   └── heading_content (inline)
│   └── paragraph
│       └── inline
├── fenced_code_block
│   ├── fenced_code_block_delimiter (```)
│   ├── info_string
│   └── code_fence_content
└── ...
```

### Extracting Headings

```rust
fn extract_headings<'a>(node: Node<'a>, source: &'a str) -> Vec<Heading<'a>> {
    let mut headings = Vec::new();
    collect_headings(node, source, &mut headings);
    headings
}

struct Heading<'a> {
    level: u8,
    text: &'a str,
    range: std::ops::Range<usize>,
}

fn collect_headings<'a>(node: Node<'a>, source: &'a str, headings: &mut Vec<Heading<'a>>) {
    if node.kind() == "atx_heading" {
        // Find the marker to determine level
        let mut level = 0u8;
        let mut text = "";

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "atx_h1_marker" => level = 1,
                "atx_h2_marker" => level = 2,
                "atx_h3_marker" => level = 3,
                "atx_h4_marker" => level = 4,
                "atx_h5_marker" => level = 5,
                "atx_h6_marker" => level = 6,
                "heading_content" | "inline" => {
                    text = child.utf8_text(source.as_bytes()).unwrap_or("");
                }
                _ => {}
            }
        }

        if level > 0 {
            headings.push(Heading {
                level,
                text: text.trim(),
                range: node.byte_range(),
            });
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_headings(child, source, headings);
    }
}
```

### Extracting Links

```rust
#[derive(Debug)]
enum Link<'a> {
    Inline {
        text: &'a str,
        url: &'a str,
        title: Option<&'a str>,
        range: std::ops::Range<usize>,
    },
    Reference {
        text: &'a str,
        label: &'a str,
        range: std::ops::Range<usize>,
    },
    Wiki {
        target: &'a str,
        alias: Option<&'a str>,
        range: std::ops::Range<usize>,
    },
}

fn extract_links<'a>(node: Node<'a>, source: &'a str) -> Vec<Link<'a>> {
    let mut links = Vec::new();
    collect_links(node, source, &mut links);
    links
}

fn collect_links<'a>(node: Node<'a>, source: &'a str, links: &mut Vec<Link<'a>>) {
    match node.kind() {
        "inline_link" => {
            let text = node.child_by_field_name("text")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let url = node.child_by_field_name("url")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");

            links.push(Link::Inline {
                text,
                url,
                title: None,
                range: node.byte_range(),
            });
        }
        "shortcut_link" | "full_reference_link" | "collapsed_reference_link" => {
            // Reference-style links
            let text = node.child_by_field_name("text")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let label = node.child_by_field_name("label")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or(text);

            links.push(Link::Reference {
                text,
                label,
                range: node.byte_range(),
            });
        }
        _ => {}
    }

    // Also check for wiki links (custom pattern in text)
    if node.kind() == "text" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        // Wiki links aren't in standard tree-sitter-markdown grammar
        // Parse manually or use regex
        for wiki in parse_wiki_links(text, node.start_byte()) {
            links.push(wiki);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_links(child, source, links);
    }
}
```

### Incremental Parsing

```rust
use tree_sitter::{InputEdit, Point};

struct Document {
    source: String,
    tree: Option<Tree>,
    parser: Parser,
}

impl Document {
    fn new(source: String) -> Self {
        let mut parser = create_parser();
        let tree = parser.parse(&source, None);
        Self { source, tree, parser }
    }

    fn apply_edit(&mut self, start_byte: usize, old_end_byte: usize, new_text: &str) {
        // Calculate positions
        let start_position = byte_to_point(&self.source, start_byte);
        let old_end_position = byte_to_point(&self.source, old_end_byte);

        // Apply text change
        let new_end_byte = start_byte + new_text.len();
        self.source.replace_range(start_byte..old_end_byte, new_text);

        let new_end_position = byte_to_point(&self.source, new_end_byte);

        // Tell tree-sitter about the edit
        if let Some(tree) = &mut self.tree {
            tree.edit(&InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            });
        }

        // Re-parse with old tree for incremental update
        self.tree = self.parser.parse(&self.source, self.tree.as_ref());
    }
}

fn byte_to_point(source: &str, byte_offset: usize) -> Point {
    let prefix = &source[..byte_offset.min(source.len())];
    let row = prefix.matches('\n').count();
    let column = prefix.rfind('\n')
        .map(|pos| byte_offset - pos - 1)
        .unwrap_or(byte_offset);
    Point { row, column }
}
```

### Using Queries

tree-sitter queries let you pattern-match on the syntax tree:

```rust
use tree_sitter::Query;

fn find_headings_with_query(tree: &Tree, source: &str) -> Vec<(u8, String, Range<usize>)> {
    let query_str = r#"
        (atx_heading
          [(atx_h1_marker) (atx_h2_marker) (atx_h3_marker)
           (atx_h4_marker) (atx_h5_marker) (atx_h6_marker)] @marker
          (heading_content) @content) @heading
    "#;

    let language = tree_sitter_markdown::language();
    let query = Query::new(&language, query_str).expect("Invalid query");

    let mut cursor = tree_sitter::QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut results = Vec::new();

    for m in matches {
        let mut level = 0u8;
        let mut text = String::new();
        let mut range = 0..0;

        for capture in m.captures {
            let name = query.capture_names()[capture.index as usize];
            let node = capture.node;

            match name {
                "marker" => {
                    level = match node.kind() {
                        "atx_h1_marker" => 1,
                        "atx_h2_marker" => 2,
                        "atx_h3_marker" => 3,
                        "atx_h4_marker" => 4,
                        "atx_h5_marker" => 5,
                        "atx_h6_marker" => 6,
                        _ => 0,
                    };
                }
                "content" => {
                    text = node.utf8_text(source.as_bytes())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                }
                "heading" => {
                    range = node.byte_range();
                }
                _ => {}
            }
        }

        if level > 0 {
            results.push((level, text, range));
        }
    }

    results
}
```

### TreeCursor for Efficient Traversal

```rust
fn find_node_at_position<'a>(tree: &'a Tree, byte_offset: usize) -> Option<Node<'a>> {
    let root = tree.root_node();
    if !root.byte_range().contains(&byte_offset) {
        return None;
    }

    let mut cursor = root.walk();
    let mut result = root;

    // Descend to most specific node
    'outer: loop {
        // Check if any child contains the position
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                if node.byte_range().contains(&byte_offset) {
                    result = node;
                    continue 'outer; // Descend into this child
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            // No child contained position, use parent
            cursor.goto_parent();
        }
        break;
    }

    Some(result)
}
```

---

## Pitfalls

### Lifetime of Nodes

<pitfall>
**Problem:** Tree nodes borrow from the Tree. Tree must outlive nodes.

```rust
// BAD: Tree dropped while nodes still in use
fn get_headings(source: &str) -> Vec<Node> {
    let mut parser = create_parser();
    let tree = parser.parse(source, None).unwrap();
    extract_nodes(&tree) // Returns Vec<Node<'_>> borrowing from tree
} // tree dropped here, nodes invalid!
```

**Solution:** Keep tree alive or extract owned data:

```rust
// GOOD: Return owned data, not borrowed nodes
fn get_headings(source: &str) -> Vec<HeadingData> {
    let mut parser = create_parser();
    let tree = parser.parse(source, None).unwrap();
    extract_nodes(&tree)
        .into_iter()
        .map(|node| HeadingData {
            text: node.utf8_text(source.as_bytes()).unwrap().to_owned(),
            range: node.byte_range(),
        })
        .collect()
}
```
</pitfall>

### Cursor Reuse

<pitfall>
**Problem:** Creating new TreeCursor for every traversal is slow.

```rust
// BAD: New cursor for each iteration
for child in node.children(&mut node.walk()) {
    for grandchild in child.children(&mut child.walk()) { // Another cursor!
        // ...
    }
}
```

**Solution:** Reuse cursor:

```rust
// GOOD: Single cursor, navigate explicitly
let mut cursor = tree.root_node().walk();
cursor.goto_first_child();
loop {
    let node = cursor.node();
    // Process node...

    // Depth-first traversal
    if cursor.goto_first_child() {
        continue;
    }
    while !cursor.goto_next_sibling() {
        if !cursor.goto_parent() {
            return; // Done
        }
    }
}
```
</pitfall>

### Byte vs Character Offsets

<pitfall>
**Problem:** tree-sitter uses byte offsets, not character counts.

```rust
// BAD: Assuming character index
let char_index = 10;
let node_text = &source[char_index..]; // Wrong if UTF-8!
```

**Solution:** Always use `node.start_byte()` / `node.end_byte()`:

```rust
// GOOD: Use byte offsets
let start = node.start_byte();
let end = node.end_byte();
let node_text = &source[start..end];
```
</pitfall>

### Edit Coordinates Must Match

<pitfall>
**Problem:** `tree.edit()` coordinates must match actual text change.

```rust
// BAD: Edit coordinates don't match actual change
self.source.replace_range(10..20, "new text"); // Changes 10 bytes
tree.edit(&InputEdit {
    start_byte: 10,
    old_end_byte: 15, // Wrong! Should be 20
    new_end_byte: 18,
    // ...
});
```

**Solution:** Calculate edit carefully:

```rust
// GOOD: Coordinates match exactly
let start = 10;
let old_end = 20; // Original range end
let new_text = "new text";
let new_end = start + new_text.len();

self.source.replace_range(start..old_end, new_text);
tree.edit(&InputEdit {
    start_byte: start,
    old_end_byte: old_end,
    new_end_byte: new_end,
    start_position: byte_to_point(&old_source, start),
    old_end_position: byte_to_point(&old_source, old_end),
    new_end_position: byte_to_point(&self.source, new_end),
});
```
</pitfall>

### Parser Is Not Thread-Safe

<pitfall>
**Problem:** `Parser` cannot be shared across threads.

```rust
// BAD: Parser shared between threads
let parser = Arc::new(Mutex::new(create_parser()));
// This works but serializes all parsing
```

**Solution:** Create parser per thread or use a pool:

```rust
// GOOD: Thread-local parser
thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(create_parser());
}

fn parse(source: &str) -> Option<Tree> {
    PARSER.with(|p| p.borrow_mut().parse(source, None))
}
```
</pitfall>

---

## Related

- LSP integration: `tower-lsp.md`
- Memory-efficient storage: `bumpalo.md`
- tree-sitter docs: https://tree-sitter.github.io/tree-sitter/
- tree-sitter-markdown: https://github.com/tree-sitter-grammars/tree-sitter-markdown
