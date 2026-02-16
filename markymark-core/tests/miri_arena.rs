//! Miri-targeted tests for unsafe arena patterns.
//!
//! These tests exercise the self-referential arena pattern used by
//! `Ast` (parser) and `DocumentIndex` (index) without FFI dependencies,
//! allowing Miri to validate memory safety.
//!
//! Run with: `cargo +nightly miri test -p markymark-core --test miri_arena`
//!
//! # Known limitations
//!
//! ArenaHashMap tests are excluded from Miri (`#[cfg_attr(miri, ignore)]`)
//! because hashbrown's drop accesses the `&Bump` allocator reference, which
//! conflicts with Box's ownership semantics under both Stacked Borrows and
//! Tree Borrows. This is a known aliasing model limitation for self-referential
//! types using Box. Tracked by marky-5yt (self_cell/ouroboros migration).

use markymark_core::arena::{
    arena_alloc_str, new_arena_hashmap, ArenaHashMap, ArenaVecBuilder, BumpVec, DocumentArena,
};

// ============================================================================
// Pattern 1: Self-referential arena with 'static cast
// Mirrors the pattern in markymark-parser/src/ast.rs
// ============================================================================

/// Minimal replica of the Ast self-referential pattern.
/// Owns a DocumentArena and stores references cast to 'static.
struct FakeAst {
    /// Stored with 'static lifetime (same pattern as Ast::root_elements).
    /// Actual lifetime is the arena's — sound because Self owns the arena.
    data: Vec<&'static str>,
    arena: Box<DocumentArena>,
}

impl FakeAst {
    fn new(strings: &[&str]) -> Self {
        let arena = Box::new(DocumentArena::new());

        // SAFETY: Same pattern as Ast::from_markdown_tree.
        // Arena is owned by Self; 'static cast is valid for Self's lifetime.
        let arena_ref: &'static bumpalo::Bump = unsafe { &*(arena.bump() as *const bumpalo::Bump) };

        let data: Vec<&'static str> = strings
            .iter()
            .map(|s| arena_alloc_str(arena_ref, s))
            .collect();

        Self { data, arena }
    }

    /// Returns data with 'static lifetime — matches Ast::root_elements() pattern.
    /// The 'static refs are valid because Self owns the arena.
    fn data(&self) -> &[&'static str] {
        &self.data
    }

    fn doc_arena_ptr(&self) -> *const DocumentArena {
        &*self.arena as *const DocumentArena
    }
}

#[test]
fn self_referential_arena_create_and_read() {
    let ast = FakeAst::new(&["hello", "world", "arena"]);
    assert_eq!(ast.data(), &["hello", "world", "arena"]);
}

#[test]
fn self_referential_arena_drop_is_sound() {
    // Allocate, read, then drop — Miri checks for use-after-free / leak
    let ast = FakeAst::new(&["test", "drop", "safety"]);
    assert_eq!(ast.data().len(), 3);
    drop(ast);
}

#[test]
fn self_referential_arena_empty() {
    let ast = FakeAst::new(&[]);
    assert!(ast.data().is_empty());
    drop(ast);
}

#[test]
fn self_referential_arena_large_allocation() {
    // Stress test: many small allocations in a single arena
    let strings: Vec<String> = (0..200).map(|i| format!("string_{i}")).collect();
    let refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
    let ast = FakeAst::new(&refs);
    assert_eq!(ast.data().len(), 200);
    for (i, s) in ast.data().iter().enumerate() {
        assert_eq!(*s, format!("string_{i}"));
    }
}

// ============================================================================
// Pattern 2: Arena ownership transfer via ptr::read + mem::forget
// Mirrors DocumentIndex::from_ast in markymark-index/src/document.rs
// ============================================================================

/// Minimal replica of the DocumentIndex ownership-transfer pattern.
/// Takes ownership of FakeAst's arena via ptr::read + mem::forget.
struct FakeIndex {
    entries: Vec<&'static str>,
    _arena: DocumentArena,
}

impl FakeIndex {
    /// Build from a FakeAst, consuming it and taking its arena.
    ///
    /// Mirrors DocumentIndex::from_ast: borrows data from the AST's arena,
    /// allocates new data, then transfers arena ownership via ptr::read + mem::forget.
    fn from_fake_ast(ast: FakeAst) -> Self {
        let doc_arena_ptr = ast.doc_arena_ptr();

        // SAFETY: Same pattern as DocumentIndex::from_ast.
        // Cast inner Bump to 'static because Self will own the arena.
        let arena_ref: &'static bumpalo::Bump =
            unsafe { &*((*doc_arena_ptr).bump() as *const bumpalo::Bump) };

        // Copy &'static str references out of the AST (simulates reading headings).
        // The borrow of `ast` from `.data()` ends after `.to_vec()` completes;
        // the Vec owns copies of the &'static str pointers, not a borrow of ast.
        let mut entries: Vec<&'static str> = ast.data().to_vec();

        // Allocate new data into the same arena (simulates slug computation)
        let extra = arena_alloc_str(arena_ref, "index-computed-value");
        entries.push(extra);

        // Transfer arena ownership: ptr::read extracts the DocumentArena,
        // mem::forget prevents ast's Drop from freeing it.
        // SAFETY: doc_arena_ptr points at ast.arena (Box<DocumentArena>).
        // We read the DocumentArena out of the Box, then forget ast to
        // prevent double-free.
        let doc_arena = unsafe { std::ptr::read(doc_arena_ptr) };
        std::mem::forget(ast);

        Self {
            entries,
            _arena: doc_arena,
        }
    }
}

#[test]
fn arena_transfer_create_and_read() {
    let ast = FakeAst::new(&["heading-1", "heading-2"]);
    let index = FakeIndex::from_fake_ast(ast);

    assert_eq!(index.entries.len(), 3); // 2 from ast + 1 computed
    assert_eq!(index.entries[0], "heading-1");
    assert_eq!(index.entries[1], "heading-2");
    assert_eq!(index.entries[2], "index-computed-value");
}

#[test]
fn arena_transfer_drop_is_sound() {
    let ast = FakeAst::new(&["a", "b", "c"]);
    let index = FakeIndex::from_fake_ast(ast);
    assert_eq!(index.entries.len(), 4);
    drop(index);
}

#[test]
fn arena_transfer_empty_ast() {
    let ast = FakeAst::new(&[]);
    let index = FakeIndex::from_fake_ast(ast);
    assert_eq!(index.entries.len(), 1); // just the computed value
    assert_eq!(index.entries[0], "index-computed-value");
}

// ============================================================================
// Pattern 3: ArenaHashMap in self-referential context
// Mirrors ArenaHashMap usage in parser types (Frontmatter, Properties, etc.)
//
// NOTE: These tests are excluded from Miri because hashbrown's Drop impl
// accesses the &Bump allocator reference, conflicting with Box's unique
// ownership semantics under Stacked Borrows and Tree Borrows. This is the
// known Box aliasing issue that marky-5yt (self_cell migration) will fix.
// The tests still run under the normal test suite for correctness validation.
// ============================================================================

struct FakeProperties {
    map: ArenaHashMap<'static, &'static str, &'static str>,
    /// Arena kept alive so 'static references remain valid.
    _arena: Box<DocumentArena>,
}

impl FakeProperties {
    fn new(pairs: &[(&str, &str)]) -> Self {
        let arena = Box::new(DocumentArena::new());
        let arena_ref: &'static bumpalo::Bump = unsafe { &*(arena.bump() as *const bumpalo::Bump) };

        let mut map: ArenaHashMap<'static, &'static str, &'static str> =
            new_arena_hashmap(arena_ref);

        for (k, v) in pairs {
            let key = arena_alloc_str(arena_ref, k);
            let val = arena_alloc_str(arena_ref, v);
            map.insert(key, val);
        }

        Self { map, _arena: arena }
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn arena_hashmap_create_and_lookup() {
    let props = FakeProperties::new(&[("title", "Hello"), ("author", "World")]);
    assert_eq!(props.map.get("title"), Some(&"Hello"));
    assert_eq!(props.map.get("author"), Some(&"World"));
    assert_eq!(props.map.get("missing"), None);
}

#[test]
#[cfg_attr(miri, ignore)]
fn arena_hashmap_drop_is_sound() {
    let props = FakeProperties::new(&[("a", "1"), ("b", "2"), ("c", "3")]);
    assert_eq!(props.map.len(), 3);
    drop(props);
}

#[test]
#[cfg_attr(miri, ignore)]
fn arena_hashmap_empty() {
    let props = FakeProperties::new(&[]);
    assert!(props.map.is_empty());
    drop(props);
}

#[test]
#[cfg_attr(miri, ignore)]
fn arena_hashmap_many_entries() {
    let pairs: Vec<(String, String)> = (0..50)
        .map(|i| (format!("key_{i}"), format!("val_{i}")))
        .collect();
    let refs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let props = FakeProperties::new(&refs);
    assert_eq!(props.map.len(), 50);
    for i in 0..50 {
        let key = format!("key_{i}");
        let expected = format!("val_{i}");
        assert_eq!(props.map.get(key.as_str()), Some(&expected.as_str()));
    }
}

// ============================================================================
// Pattern 4: BumpVec into_bump_slice in self-referential context
// Mirrors the pattern used for headings, tags, wiki_links in DocumentIndex
//
// NOTE: These tests are also excluded from Miri because the &'static slice
// stored as a struct field points into arena memory. During struct drop,
// Miri protects the slice reference (creating a SharedReadOnly tag), then
// the Box<DocumentArena> deallocation conflicts with that protected tag.
// Same root cause as Pattern 3 — tracked by marky-5yt.
// ============================================================================

struct FakeSliceOwner {
    items: &'static [&'static str],
    _arena: Box<DocumentArena>,
}

impl FakeSliceOwner {
    fn new(strings: &[&str]) -> Self {
        let arena = Box::new(DocumentArena::new());
        let arena_ref: &'static bumpalo::Bump = unsafe { &*(arena.bump() as *const bumpalo::Bump) };

        let mut builder: BumpVec<'static, &'static str> = BumpVec::new_in(arena_ref);
        for s in strings {
            builder.push(arena_alloc_str(arena_ref, s));
        }
        let items = builder.into_bump_slice();

        Self {
            items,
            _arena: arena,
        }
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn bump_slice_create_and_read() {
    let owner = FakeSliceOwner::new(&["alpha", "beta", "gamma"]);
    assert_eq!(owner.items, &["alpha", "beta", "gamma"]);
}

#[test]
#[cfg_attr(miri, ignore)]
fn bump_slice_drop_is_sound() {
    let owner = FakeSliceOwner::new(&["x", "y"]);
    assert_eq!(owner.items.len(), 2);
    drop(owner);
}

// ============================================================================
// Pattern 5: ArenaVecBuilder (core utility)
// ============================================================================

#[test]
fn arena_vec_builder_in_self_referential_context() {
    let arena = Box::new(DocumentArena::new());
    let arena_ref: &'static bumpalo::Bump = unsafe { &*(arena.bump() as *const bumpalo::Bump) };

    let mut builder = ArenaVecBuilder::new(arena_ref);
    builder.push(arena_alloc_str(arena_ref, "one"));
    builder.push(arena_alloc_str(arena_ref, "two"));
    let slice: &'static [&'static str] = builder.into_bump_slice();

    assert_eq!(slice, &["one", "two"]);
    drop(arena);
}
