# bumpalo - Arena Allocation

<agent>
<goal>Use arena allocation for fast, bulk-deallocated memory management.</goal>
<when_to_use>When you need many small allocations that can be freed together (parsing, indexing, per-request state).</when_to_use>
<contains>Bump allocator setup, arena-allocated strings/vecs, lifetimes, bulk deallocation patterns</contains>
<see_also>tree-sitter.md, petgraph.md</see_also>
</agent>

**TL;DR:** bumpalo provides bump allocation - allocations are fast (pointer bump), individual frees are impossible, drop the arena to free everything at once. Perfect for per-document or per-realm state in LSPs.

**Checklist:**
- [ ] Create `Bump` allocator for each scope (document, realm, request)
- [ ] Use `bump.alloc()` for single values, `bump.alloc_slice_copy()` for slices
- [ ] Use `bumpalo::collections::Vec` / `String` for growable collections
- [ ] Drop entire `Bump` to free all memory at once
- [ ] Data lifetimes tied to arena lifetime (`'arena`)

---

## Setup

### Cargo.toml

```toml
[dependencies]
bumpalo = { version = "3.16", features = ["collections", "boxed", "allocator-api2"] }
```

Features:
- **collections** - Vec, String, and other collection types in the arena
- **boxed** - Drop handling for heap/box-like complex types
- **allocator-api2** - Custom allocator support so types like hashbrown can use the arena

### Basic Usage

```rust
use bumpalo::Bump;

fn main() {
    // Create arena
    let arena = Bump::new();

    // Allocate single value
    let x: &mut i32 = arena.alloc(42);
    *x += 1;
    println!("x = {}", x);

    // Allocate struct
    let point: &mut Point = arena.alloc(Point { x: 1.0, y: 2.0 });

    // Allocate slice from iterator
    let nums: &[i32] = arena.alloc_slice_fill_iter(0..10);
    println!("nums = {:?}", nums);

    // Allocate string
    let s: &str = arena.alloc_str("hello");
    println!("s = {}", s);

    // When arena is dropped, all memory freed at once
}

struct Point {
    x: f64,
    y: f64,
}
```

---

## Patterns

### Per-Document Arena (LSP Pattern)

```rust
use bumpalo::Bump;
use std::collections::HashMap;

struct Document<'arena> {
    arena: &'arena Bump,
    uri: &'arena str,
    content: &'arena str,
    headings: &'arena [Heading<'arena>],
    links: &'arena [Link<'arena>],
}

struct Heading<'arena> {
    level: u8,
    text: &'arena str,
    slug: &'arena str,
    range: Range<usize>,
}

struct Link<'arena> {
    target: &'arena str,
    anchor: Option<&'arena str>,
    range: Range<usize>,
}

impl<'arena> Document<'arena> {
    fn parse(arena: &'arena Bump, uri: &str, content: &str) -> Self {
        // Allocate strings in arena
        let uri = arena.alloc_str(uri);
        let content = arena.alloc_str(content);

        // Parse and allocate headings
        let headings = parse_headings(arena, content);
        let links = parse_links(arena, content);

        Self { arena, uri, content, headings, links }
    }
}

fn parse_headings<'arena>(arena: &'arena Bump, content: &str) -> &'arena [Heading<'arena>] {
    let mut headings = bumpalo::collections::Vec::new_in(arena);

    // ... parsing logic ...
    // For each heading found:
    headings.push(Heading {
        level: 1,
        text: arena.alloc_str("Example"),
        slug: arena.alloc_str("example"),
        range: 0..10,
    });

    headings.into_bump_slice()
}

fn parse_links<'arena>(arena: &'arena Bump, content: &str) -> &'arena [Link<'arena>] {
    let mut links = bumpalo::collections::Vec::new_in(arena);
    // ... parsing logic ...
    links.into_bump_slice()
}
```

### Per-Realm Arena (Multi-tenant LSP)

```rust
use bumpalo::Bump;
use std::collections::HashMap;

struct Realm {
    id: RealmId,
    arena: Bump,  // Owned, not borrowed
    documents: HashMap<String, DocumentIndex>,
}

struct DocumentIndex {
    // Indices into arena-allocated data
    heading_count: usize,
    link_count: usize,
    // ... other metadata
}

impl Realm {
    fn new(id: RealmId) -> Self {
        Self {
            id,
            arena: Bump::new(),
            documents: HashMap::new(),
        }
    }

    fn add_document(&mut self, uri: &str, content: &str) {
        // Parse into arena
        let heading_count = self.parse_headings(content);
        let link_count = self.parse_links(content);

        self.documents.insert(uri.to_string(), DocumentIndex {
            heading_count,
            link_count,
        });
    }

    fn destroy(self) {
        // Drop self, arena dropped, all memory freed instantly
        // O(1) cleanup regardless of document count
    }

    fn reset(&mut self) {
        // Reset arena, keeps capacity allocated
        self.arena.reset();
        self.documents.clear();
    }

    fn parse_headings(&self, content: &str) -> usize {
        // Use &self.arena for allocations
        // Returns count since actual data is in arena
        0
    }

    fn parse_links(&self, content: &str) -> usize {
        0
    }
}

// Realm destruction is O(1)
fn benchmark_realm_cleanup() {
    let mut realm = Realm::new(RealmId(1));

    // Add 10,000 documents
    for i in 0..10_000 {
        realm.add_document(&format!("doc{}.md", i), "# Heading\nContent...");
    }

    // O(1) cleanup - just drops arena
    drop(realm);
}
```

### Arena-Allocated Collections

```rust
use bumpalo::Bump;
use bumpalo::collections::{Vec as BumpVec, String as BumpString};

fn arena_collections(arena: &Bump) {
    // Vec in arena
    let mut vec: BumpVec<i32> = BumpVec::new_in(arena);
    vec.push(1);
    vec.push(2);
    vec.push(3);

    // Convert to slice (no reallocation)
    let slice: &[i32] = vec.into_bump_slice();

    // String in arena
    let mut s: BumpString = BumpString::new_in(arena);
    s.push_str("hello");
    s.push_str(" world");

    // Convert to str slice
    let str_slice: &str = s.into_bump_str();

    // HashMap with arena-allocated keys/values
    // Note: HashMap itself isn't arena-allocated, but its contents can be
    use std::collections::HashMap;
    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert(arena.alloc_str("key"), arena.alloc_str("value"));
}
```

### Formatting into Arena

```rust
use bumpalo::Bump;
use std::fmt::Write;

fn format_in_arena<'a>(arena: &'a Bump, name: &str, count: usize) -> &'a str {
    let mut s = bumpalo::collections::String::new_in(arena);
    write!(s, "{}: {} items", name, count).unwrap();
    s.into_bump_str()
}

// Or use bumpalo::format! macro
fn format_macro<'a>(arena: &'a Bump, name: &str) -> &'a str {
    bumpalo::format!(in arena, "Hello, {}!", name).into_bump_str()
}
```

### Allocating Complex Structures

```rust
use bumpalo::Bump;

#[derive(Debug)]
struct AstNode<'arena> {
    kind: NodeKind<'arena>,
    children: &'arena [AstNode<'arena>],
    range: Range<usize>,
}

#[derive(Debug)]
enum NodeKind<'arena> {
    Document,
    Heading { level: u8, text: &'arena str },
    Paragraph,
    Link { target: &'arena str, text: &'arena str },
    Text(&'arena str),
}

fn build_ast<'arena>(arena: &'arena Bump, source: &str) -> &'arena AstNode<'arena> {
    // Build children first
    let heading = arena.alloc(AstNode {
        kind: NodeKind::Heading {
            level: 1,
            text: arena.alloc_str("Title"),
        },
        children: &[],
        range: 0..8,
    });

    let paragraph = arena.alloc(AstNode {
        kind: NodeKind::Paragraph,
        children: arena.alloc_slice_copy(&[
            AstNode {
                kind: NodeKind::Text(arena.alloc_str("Some text")),
                children: &[],
                range: 10..19,
            },
        ]),
        range: 10..19,
    });

    // Build document with children
    arena.alloc(AstNode {
        kind: NodeKind::Document,
        children: arena.alloc_slice_copy(&[*heading, *paragraph]),
        range: 0..source.len(),
    })
}
```

### Memory Usage Tracking

```rust
use bumpalo::Bump;

fn track_memory() {
    let arena = Bump::new();

    println!("Initial: {} bytes allocated", arena.allocated_bytes());

    let _data: &[u8] = arena.alloc_slice_fill_copy(1000, 0u8);
    println!("After 1KB: {} bytes allocated", arena.allocated_bytes());

    // Allocated includes internal fragmentation/alignment
    // Actual used memory may be less

    // Reset keeps capacity but allows reuse
    // arena.reset(); // Would reset to 0 used
}
```

---

## Pitfalls

### Lifetimes Must Match Arena

<pitfall>
**Problem:** Data allocated in arena cannot outlive the arena.

```rust
// BAD: Returns reference to arena-local data
fn parse_heading(content: &str) -> &str {
    let arena = Bump::new();
    let heading = arena.alloc_str(&content[0..10]);
    heading // ERROR: arena dropped, heading invalid!
}
```

**Solution:** Pass arena from caller or return owned data:

```rust
// GOOD: Arena lifetime flows through
fn parse_heading<'a>(arena: &'a Bump, content: &str) -> &'a str {
    arena.alloc_str(&content[0..10])
}

// Or return owned
fn parse_heading_owned(content: &str) -> String {
    content[0..10].to_string()
}
```
</pitfall>

### No Individual Deallocation

<pitfall>
**Problem:** Cannot free individual allocations.

```rust
// BAD: Expecting to free individual items
let arena = Bump::new();
let a = arena.alloc(1);
let b = arena.alloc(2);
// Cannot free just 'a' - must drop entire arena

// This is a design feature, not a bug!
```

**Solution:** Design around bulk deallocation:

```rust
// GOOD: One arena per logical unit that's freed together
struct Request {
    arena: Bump,
    // ... request-scoped data
}

// When request completes, drop Request, arena freed
```
</pitfall>

### Drop Not Called

<pitfall>
**Problem:** `Bump::alloc` doesn't call `Drop` on allocated values.

```rust
struct NeedsDrop {
    data: String, // Has Drop impl
}

let arena = Bump::new();
let x = arena.alloc(NeedsDrop { data: "hello".into() });
// When arena dropped, NeedsDrop::drop() is NOT called!
// The String inside is leaked!
```

**Solution:** Only allocate types that don't need Drop, or use `alloc_with`:

```rust
// GOOD: Use types without Drop
struct NoDrop<'a> {
    data: &'a str, // Borrowed, no Drop
}

// Or use bumpalo's collections which handle this
use bumpalo::collections::String as BumpString;
let s = BumpString::from_str_in("hello", &arena);
// BumpString doesn't heap-allocate, so no leak
```

For types that need Drop, use `bumpalo::boxed::Box`:

```rust
use bumpalo::boxed::Box as BumpBox;

let arena = Bump::new();
let boxed = BumpBox::new_in(NeedsDrop { data: "hello".into() }, &arena);
// Drop WILL be called when arena is dropped (with "collections" feature)
```
</pitfall>

### Thread Safety

<pitfall>
**Problem:** `Bump` is not `Sync` - cannot be shared across threads.

```rust
// BAD: Sharing arena across threads
let arena = Arc::new(Bump::new());
let arena_clone = arena.clone();
std::thread::spawn(move || {
    arena_clone.alloc(1); // ERROR: Bump is not Sync
});
```

**Solution:** Use one arena per thread or protect with mutex (defeats purpose):

```rust
// GOOD: Thread-local arenas
thread_local! {
    static ARENA: RefCell<Bump> = RefCell::new(Bump::new());
}

fn alloc_in_thread<T>(value: T) -> *mut T {
    ARENA.with(|arena| {
        arena.borrow().alloc(value) as *mut T
    })
}
```
</pitfall>

### Arena Capacity Growth

<pitfall>
**Problem:** Arena grows but never shrinks until dropped.

```rust
let arena = Bump::new();
// Allocate 1GB
let _big = arena.alloc_slice_fill_copy(1_000_000_000, 0u8);
arena.reset(); // Frees logically, but capacity stays at 1GB!
```

**Solution:** Drop and recreate arena to release memory:

```rust
// If you need to release memory:
fn process_with_fresh_arena() {
    let arena = Bump::new();
    // ... use arena ...
} // Arena dropped, memory released

// Or use with_capacity for predictable size:
let arena = Bump::with_capacity(1024 * 1024); // 1MB initial
```
</pitfall>

---

## Related

- Parser output storage: `tree-sitter.md`
- Graph with arena nodes: `petgraph.md`
- bumpalo docs: https://docs.rs/bumpalo/
- bumpalo collections: https://docs.rs/bumpalo/latest/bumpalo/collections/index.html
