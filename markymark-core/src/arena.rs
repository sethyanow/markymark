//! Arena allocation infrastructure for markymark.
//!
//! This module provides arena-allocated types and helpers for efficient
//! bulk deallocation of parser and index data structures. Shared vocabulary
//! for all crates: ArenaStr, ArenaVec, ArenaHashMap, and DocumentArena.

pub use bumpalo::boxed::Box as BumpBox;
pub use bumpalo::collections::{String as BumpString, Vec as BumpVec};
pub use bumpalo::Bump;

pub use hashbrown::HashMap;

// ============================================================================
// TYPE ALIASES — shared vocabulary for arena-allocated types
// ============================================================================

/// Arena-allocated string reference.
///
/// Use `arena.alloc_str(s)` to produce; all contents share the arena lifetime.
pub type ArenaStr<'a> = &'a str;

/// Arena-allocated slice.
///
/// Use `BumpVec::new_in(arena)` and `.into_bump_slice()` to produce.
pub type ArenaSlice<'a, T> = &'a [T];

/// Arena-allocated HashMap (dec-arena-003).
///
/// Keys, values, and internal buckets are allocated in the arena.
/// Use [`new_arena_hashmap`] to construct.
pub type ArenaHashMap<'a, K, V> = HashMap<K, V, hashbrown::DefaultHashBuilder, &'a Bump>;

/// Create an arena-allocated HashMap.
///
/// Map's internal storage is allocated in the arena; O(1) bulk deallocation
/// when the arena is dropped.
pub fn new_arena_hashmap<'a, K, V>(arena: &'a Bump) -> ArenaHashMap<'a, K, V> {
    HashMap::new_in(arena)
}

// ============================================================================
// DocumentArena — per-document bump allocator wrapper
// ============================================================================

/// Per-document arena for parsed content.
///
/// Wraps a `Bump` allocator scoped to a single document. When the document
/// is closed or reparsed, dropping this arena frees all allocations at once.
#[derive(Debug, Default)]
pub struct DocumentArena(Bump);

impl DocumentArena {
    /// Create a new document arena.
    pub fn new() -> Self {
        Self(Bump::new())
    }

    /// Create with an initial capacity hint.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Bump::with_capacity(capacity))
    }

    /// Borrow the inner bump allocator.
    pub fn bump(&self) -> &Bump {
        &self.0
    }
}

impl AsRef<Bump> for DocumentArena {
    fn as_ref(&self) -> &Bump {
        &self.0
    }
}

// ============================================================================
// Helper trait for allocating strings in an arena
// ============================================================================

/// Helper trait for allocating strings in an arena.
pub trait ArenaStringExt {
    /// Allocate a string slice in the arena.
    fn alloc_str(&self, s: &str) -> &str;
}

impl ArenaStringExt for Bump {
    fn alloc_str(&self, s: &str) -> &str {
        Bump::alloc_str(self, s)
    }
}

// ============================================================================
// ArenaVecBuilder — helper for building arena-allocated vectors
// ============================================================================

/// Helper for building arena-allocated vectors.
pub struct ArenaVecBuilder<'a, T> {
    inner: BumpVec<'a, T>,
}

impl<'a, T> ArenaVecBuilder<'a, T> {
    /// Create a new vector builder with arena allocation.
    pub fn new(arena: &'a Bump) -> Self {
        Self {
            inner: BumpVec::new_in(arena),
        }
    }

    /// Push an element.
    pub fn push(&mut self, value: T) {
        self.inner.push(value);
    }

    /// Convert to an arena-allocated slice.
    pub fn into_bump_slice(self) -> &'a [T] {
        self.inner.into_bump_slice()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod arena_allocation_tests {
    use super::*;

    /// Verify Bump is available and works
    #[test]
    fn bump_allocator_works() {
        let arena = Bump::new();
        let s: ArenaStr<'_> = arena.alloc_str("hello");
        assert_eq!(s, "hello");
    }

    /// Verify BumpVec works and produces slices
    #[test]
    fn bump_vec_produces_slices() {
        let arena = Bump::new();
        let mut builder = ArenaVecBuilder::new(&arena);
        builder.push(1);
        builder.push(2);
        builder.push(3);
        let slice: ArenaSlice<'_, i32> = builder.into_bump_slice();
        assert_eq!(slice, &[1, 2, 3]);
    }

    /// Core provides ArenaHashMap with bumpalo allocator
    #[test]
    fn core_provides_arena_hashmap() {
        let arena = Bump::new();
        let mut map: ArenaHashMap<'_, &str, i32> = new_arena_hashmap(&arena);
        map.insert(arena.alloc_str("one"), 1);
        map.insert(arena.alloc_str("two"), 2);

        assert_eq!(map.get("one"), Some(&1));
        assert_eq!(map.get("two"), Some(&2));
    }

    /// DocumentArena provides bump access
    #[test]
    fn document_arena_provides_bump() {
        let doc_arena = DocumentArena::new();
        let s: ArenaStr<'_> = doc_arena.bump().alloc_str("test");
        assert_eq!(s, "test");
    }

    /// ArenaStr is the shared type alias for arena string refs
    #[test]
    fn arena_str_type_alias_works() {
        let arena = Bump::new();
        let _s: ArenaStr<'_> = arena.alloc_str("test");
    }
}
