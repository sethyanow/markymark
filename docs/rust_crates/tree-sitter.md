# tree-sitter - Incremental Parsing

<agent>
<goal>Parse markdown and other languages incrementally with tree-sitter bindings.</goal>
<when_to_use>When you need fast, incremental parsing with syntax tree access.</when_to_use>
<contains>Parser setup, tree-sitter-md (MarkdownParser wrapper), node traversal, queries, incremental updates, edit application</contains>
<see_also>tower-lsp.md, bumpalo.md</see_also>
</agent>

**TL;DR:** tree-sitter provides incremental parsing. For markdown, use `tree_sitter_md::MarkdownParser` which handles block+inline grammars automatically. For other languages, create a `Parser` with a `Language` from a grammar crate's `LANGUAGE` constant. Parse text to get a Tree, traverse Nodes. On edits, call `edit()` then re-parse for O(edit size) updates.

**Checklist:**
- [ ] Use `MarkdownParser::default()` for markdown (not raw `Parser`)
- [ ] For other grammars: `LANGUAGE.into()` for Language, `set_language(&lang)` by reference
- [ ] Parse text with `parser.parse(text.as_bytes(), old_tree)`
- [ ] Traverse block tree with `md_tree.block_tree().root_node()`
- [ ] Handle `section` nodes wrapping content in tree-sitter-md
- [ ] Ensure source text ends with `\n` (tree-sitter-md requirement)

---

## Setup

### Cargo.toml

```toml
[dependencies]
tree-sitter = "0.26"
tree-sitter-md = { version = "0.5", features = ["parser"] }
tree-sitter-json = "0.24"  # For JSON parsing
```

Grammar crates use `tree-sitter-language` as a bridge crate, so they work with any tree-sitter >= 0.24.

### Markdown Parsing (MarkdownParser wrapper)

```rust
use tree_sitter_md::{MarkdownParser, MarkdownTree};
use tree_sitter::Node;

fn parse_markdown(source: &str) -> Option<MarkdownTree> {
    let mut parser = MarkdownParser::default();
    // MarkdownParser::parse takes &[u8], not &str
    // Source MUST end with \n or tree-sitter-md produces ERROR nodes
    parser.parse(source.as_bytes(), None)
}

fn main() {
    let source = "# Hello\n\nSome text with a [[wiki link]].\n";
    if let Some(md_tree) = parse_markdown(source) {
        let root = md_tree.block_tree().root_node();
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

### Other Language Parsing (e.g., JSON)

```rust
use tree_sitter::{Parser, Node};

fn create_json_parser() -> Parser {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_json::LANGUAGE.into();
    parser
        .set_language(&language)  // Takes &Language in 0.26
        .expect("Error loading JSON grammar");
    parser
}
```

---

## Patterns

### tree-sitter-md Node Types (Block Grammar)

tree-sitter-md uses a two-grammar architecture (block + inline). The block grammar wraps content in `section` nodes:

```
document
└── section                          <-- wraps heading + its content
    ├── atx_heading
    │   ├── atx_h1_marker (#)
    │   └── inline (heading text)    <-- "inline", not "heading_content"
    ├── paragraph
    │   └── inline
    ├── list                         <-- single "list" type (no tight_list/loose_list)
    │   ├── list_item
    │   │   ├── list_marker_minus (- )
    │   │   └── paragraph
    │   └── list_item
    ├── fenced_code_block
    │   ├── fenced_code_block_delimiter
    │   ├── info_string
    │   └── code_fence_content
    └── section                      <-- nested section for sub-headings
        ├── atx_heading
        │   ├── atx_h2_marker (##)
        │   └── inline
        └── paragraph
```

**Key differences from old tree-sitter-markdown 0.7:**

| Old (0.7) | New (tree-sitter-md 0.5) | Notes |
|-----------|--------------------------|-------|
| No section wrapping | `section` wraps headings + content | Must recurse into sections |
| `heading_content` | `inline` | Child node name changed |
| `tight_list` / `loose_list` | `list` | Single list node type |
| `language()` function | `LANGUAGE` constant (`LanguageFn`) | Use `.into()` for Language |
| Heading node excludes `\n` | Heading node includes trailing `\n` | Affects range calculations |

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
                "inline" => {
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

    // Recurse into sections and other nodes
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

    // Wiki links aren't in the tree-sitter grammar — parsed via regex
    if node.kind() == "text" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
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

### Incremental Parsing with MarkdownTree

```rust
use tree_sitter::InputEdit;
use tree_sitter_md::{MarkdownParser, MarkdownTree};

struct Document {
    source: String,
    md_tree: Option<MarkdownTree>,
    parser: MarkdownParser,
}

impl Document {
    fn new(source: String) -> Self {
        let mut parser = MarkdownParser::default();
        let md_tree = parser.parse(source.as_bytes(), None);
        Self { source, md_tree, parser }
    }

    fn apply_edit(&mut self, start_byte: usize, old_end_byte: usize, new_text: &str) {
        let start_position = byte_to_point(&self.source, start_byte);
        let old_end_position = byte_to_point(&self.source, old_end_byte);

        let new_end_byte = start_byte + new_text.len();
        self.source.replace_range(start_byte..old_end_byte, new_text);
        let new_end_position = byte_to_point(&self.source, new_end_byte);

        // MarkdownTree.edit() takes a slice of InputEdit
        if let Some(md_tree) = &mut self.md_tree {
            md_tree.edit(&[InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            }]);
        }

        // Re-parse with old tree for incremental update
        self.md_tree = self.parser.parse(
            self.source.as_bytes(),
            self.md_tree.as_ref(),
        );
    }
}

fn byte_to_point(source: &str, byte_offset: usize) -> tree_sitter::Point {
    let prefix = &source[..byte_offset.min(source.len())];
    let row = prefix.matches('\n').count();
    let column = prefix.rfind('\n')
        .map(|pos| byte_offset - pos - 1)
        .unwrap_or(byte_offset);
    tree_sitter::Point { row, column }
}
```

### Using Queries

tree-sitter queries pattern-match on the syntax tree:

```rust
use tree_sitter::Query;
use tree_sitter_md::LANGUAGE;

fn find_headings_with_query(
    tree: &tree_sitter::Tree,
    source: &str,
) -> Vec<(u8, String, std::ops::Range<usize>)> {
    let query_str = r#"
        (atx_heading
          [(atx_h1_marker) (atx_h2_marker) (atx_h3_marker)
           (atx_h4_marker) (atx_h5_marker) (atx_h6_marker)] @marker
          (inline) @content) @heading
    "#;

    let language: tree_sitter::Language = LANGUAGE.into();
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
fn find_node_at_position<'a>(tree: &'a tree_sitter::Tree, byte_offset: usize) -> Option<Node<'a>> {
    let root = tree.root_node();
    if !root.byte_range().contains(&byte_offset) {
        return None;
    }

    let mut cursor = root.walk();
    let mut result = root;

    // Descend to most specific node
    'outer: loop {
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                if node.byte_range().contains(&byte_offset) {
                    result = node;
                    continue 'outer;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
        break;
    }

    Some(result)
}
```

---

## Pitfalls

### Trailing Newline Required

<pitfall>
**Problem:** tree-sitter-md requires source text to end with `\n`. Without it, block-level elements produce ERROR nodes.

```rust
// BAD: No trailing newline
let source = "# Hello";
let tree = parser.parse(source.as_bytes(), None); // ERROR node!
```

**Solution:** Normalize input before parsing:

```rust
// GOOD: Ensure trailing newline
let source = "# Hello";
let normalized = if source.ends_with('\n') {
    source.to_string()
} else {
    format!("{source}\n")
};
let tree = parser.parse(normalized.as_bytes(), None); // Valid tree
```
</pitfall>

### Section Node Wrapping

<pitfall>
**Problem:** tree-sitter-md wraps content in `section` nodes. Direct children of `document` are sections, not headings/paragraphs.

```rust
// BAD: Expecting headings as direct children of root
let root = md_tree.block_tree().root_node();
for child in root.children(&mut root.walk()) {
    if child.kind() == "atx_heading" { // Never matches! Headings are inside sections
        // ...
    }
}
```

**Solution:** Recurse into section nodes:

```rust
// GOOD: Handle section wrapping
fn collect_elements(node: Node, source: &str) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "section" => collect_elements(child, source), // Recurse
            "atx_heading" => { /* process heading */ }
            "paragraph" => { /* process paragraph */ }
            "list" => { /* process list */ }
            _ => {}
        }
    }
}
```
</pitfall>

### Lifetime of Nodes

<pitfall>
**Problem:** Tree nodes borrow from the Tree. Tree must outlive nodes.

```rust
// BAD: Tree dropped while nodes still in use
fn get_headings(source: &str) -> Vec<Node> {
    let mut parser = MarkdownParser::default();
    let md_tree = parser.parse(source.as_bytes(), None).unwrap();
    let root = md_tree.block_tree().root_node();
    extract_nodes(&root) // Returns Vec<Node<'_>> borrowing from tree
} // md_tree dropped here, nodes invalid!
```

**Solution:** Keep tree alive or extract owned data:

```rust
// GOOD: Return owned data, not borrowed nodes
fn get_headings(source: &str) -> Vec<HeadingData> {
    let mut parser = MarkdownParser::default();
    let md_tree = parser.parse(source.as_bytes(), None).unwrap();
    let root = md_tree.block_tree().root_node();
    extract_nodes(&root)
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
**Problem:** `tree.edit()` / `md_tree.edit()` coordinates must match actual text change.

```rust
// BAD: Edit coordinates don't match actual change
self.source.replace_range(10..20, "new text");
md_tree.edit(&[InputEdit {
    start_byte: 10,
    old_end_byte: 15, // Wrong! Should be 20
    new_end_byte: 18,
    // ...
}]);
```

**Solution:** Calculate edit carefully:

```rust
// GOOD: Coordinates match exactly
let start = 10;
let old_end = 20;
let new_text = "new text";
let new_end = start + new_text.len();

self.source.replace_range(start..old_end, new_text);
md_tree.edit(&[InputEdit {
    start_byte: start,
    old_end_byte: old_end,
    new_end_byte: new_end,
    start_position: byte_to_point(&old_source, start),
    old_end_position: byte_to_point(&old_source, old_end),
    new_end_position: byte_to_point(&self.source, new_end),
}]);
```
</pitfall>

### Parser Is Not Thread-Safe

<pitfall>
**Problem:** `MarkdownParser` and `Parser` cannot be shared across threads.

```rust
// BAD: Parser shared between threads
let parser = Arc::new(Mutex::new(MarkdownParser::default()));
// This works but serializes all parsing
```

**Solution:** Create parser per thread or use a pool:

```rust
// GOOD: Thread-local parser
thread_local! {
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

fn parse(source: &str) -> Option<MarkdownTree> {
    PARSER.with(|p| p.borrow_mut().parse(source.as_bytes(), None))
}
```
</pitfall>

---

## Related

- LSP integration: `tower-lsp.md`
- Memory-efficient storage: `bumpalo.md`
- tree-sitter docs: https://tree-sitter.github.io/tree-sitter/
- tree-sitter-md (markdown): https://github.com/tree-sitter-grammars/tree-sitter-markdown
- tree-sitter-json: https://github.com/nickel-lang/tree-sitter-json
