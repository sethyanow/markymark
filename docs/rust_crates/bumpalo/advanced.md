# bumpalo Advanced Patterns

Real-world patterns from markymark arena migration addressing cases the textbook `'arena` lifetime pattern cannot express.

[← Back to bumpalo.md](../bumpalo.md)

---

## Self-Referential Arena Ownership

When a struct **owns** the arena and also stores references into it, you cannot
express the lifetime with a parameter (`'arena`), because the struct would need
to borrow from itself. The workaround is `'static` with raw-pointer casts.

```rust
use markymark_core::arena::DocumentArena;

/// Owns its arena; stores references as 'static (valid for Self's lifetime).
pub struct Ast {
    arena: DocumentArena,
    // 'static is a lie — actually valid for arena's lifetime, which is Self's.
    root_elements: Vec<Element<'static>>,
}

impl Ast {
    /// Internal: cast arena ref to 'static for self-referential storage.
    fn arena_ref(&self) -> &'static bumpalo::Bump {
        // SAFETY: arena is owned by Self; ref valid for Self's lifetime.
        unsafe { &*(self.arena.bump() as *const bumpalo::Bump) }
    }
}
```

**Safety invariants:**
1. No `'static` references may escape beyond `&self` method returns.
2. The arena must not be dropped, moved, or reset while references exist.
3. The struct must not implement `Clone` (would create aliased arenas).

## Arena Transfer (ptr::read + mem::forget)

When building a second struct (e.g. `DocumentIndex`) that borrows from the
first struct's arena (e.g. `Ast`) during construction, then needs to **take
ownership** of that arena:

```rust
pub fn from_ast(ast: Ast) -> DocumentIndex {
    // 1. Get raw pointer to the owned DocumentArena
    let doc_arena_ptr = ast.doc_arena_ptr();
    // 2. Borrow inner Bump for allocations (cast to 'static)
    let arena_ref: &'static Bump =
        unsafe { &*((*doc_arena_ptr).bump() as *const Bump) };

    // 3. Build index data using arena_ref ...
    let headings = /* ... allocate in arena_ref ... */;

    // 4. Move arena ownership: read DocumentArena out, forget Ast shell
    let doc_arena = unsafe { std::ptr::read(doc_arena_ptr) };
    std::mem::forget(ast);

    DocumentIndex { _arena: Mutex::new(doc_arena), headings, /* ... */ }
}
```

**Why Mutex?** `Bump: !Sync` means `DocumentArena: !Sync`. Wrapping in `Mutex`
makes `DocumentIndex` `Send + Sync` for async LSP contexts (tower-lsp requires it).

## Hybrid Ownership Model

Per-document arena for parsed content; owned `String` for cross-document lookups.

```text
┌─────────────────────────────────────┐
│ RealmIndex (owns cross-doc state)   │
│  slug_to_headings: HashMap<String,  │ ← owned String keys
│    Vec<(DocumentUri, Resolved...)>> │ ← owned copies for survival
│                                     │
│  docs: HashMap<String,              │
│    (DocumentUri, DocumentIndex)>    │
│      └─ _arena: Mutex<DocArena>    │ ← per-doc arena
│         headings: &'static [...]   │ ← borrows from arena
│         wiki_links: &'static [...] │
└─────────────────────────────────────┘
```

- **Per-document arena**: headings, slugs, links, tags borrow from arena `&str`.
- **Cross-document**: `RealmIndex` stores **owned** copies (`String`, not `&str`)
  so lookups survive document removal/replacement.

## hashbrown with Arena Allocator (ArenaHashMap)

Parser types (e.g. `Frontmatter`, `XmlTag`) use `ArenaHashMap` where the map's
internal buckets are arena-allocated alongside keys/values:

```rust
use markymark_core::arena::{ArenaHashMap, new_arena_hashmap};

struct Frontmatter<'arena> {
    data: ArenaHashMap<'arena, &'arena str, FrontmatterValue<'arena>>,
}

fn parse_frontmatter<'a>(arena: &'a Bump) -> Frontmatter<'a> {
    let mut data = new_arena_hashmap(arena);
    data.insert(arena.alloc_str("title"), /* ... */);
    Frontmatter { data }
}
```

## Send Constraint

<pitfall>
**Problem:** `Bump: !Sync` → `&Bump: !Send` → `ArenaHashMap: !Send`.

Any type containing `ArenaHashMap` cannot satisfy `Send`, which tower-lsp
requires for async LSP handlers.

**Rule of thumb:**
- **Parser types** (transient, consumed by `from_ast`): **can** use `ArenaHashMap`.
- **Index types** (stored in `DocumentIndex` → `RealmIndex` → `ServerState`): **must** use standard `HashMap` with arena-borrowed keys/values.

```rust
// GOOD: Index type uses std HashMap, keys borrow from arena
pub struct XmlTagEntry<'arena> {
    pub tag_name: &'arena str,
    pub attributes: HashMap<&'arena str, &'arena str>,  // std HashMap, Send-safe
}

// GOOD: Parser type uses ArenaHashMap (never stored long-term)
pub struct XmlTag<'arena> {
    pub tag_name: &'arena str,
    pub attributes: ArenaHashMap<'arena, &'arena str, &'arena str>,  // arena-allocated
}
```
</pitfall>

---

## Related

- [bumpalo.md](../bumpalo.md) - Main documentation
- [pitfalls.md](pitfalls.md) - Common mistakes
- markymark-core/src/arena.rs - DocumentArena wrapper
- markymark-parser/src/ast.rs - Self-referential Ast example
- markymark-index/src/document.rs - Arena transfer example
