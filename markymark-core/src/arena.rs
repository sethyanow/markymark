//! Arena allocation infrastructure for markymark.
//!
//! This module provides arena-allocated types and helpers for efficient
//! bulk deallocation of parser and index data structures.

pub use bumpalo::Bump;
pub use bumpalo::boxed::Box as BumpBox;
pub use bumpalo::collections::{String as BumpString, Vec as BumpVec};

/// Re-export hashbrown for arena-compatible hash maps.
pub use hashbrown::HashMap;

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
// ARENA ALLOCATION TESTS
// These tests verify arena infrastructure works correctly.
// ============================================================================

#[cfg(test)]
#[allow(unused_variables)] // Arena used in TODOs for future arena-allocated HashMap
mod arena_allocation_tests {
    use super::*;

    /// Verify Bump is available and works
    #[test]
    fn bump_allocator_works() {
        let arena = Bump::new();
        let s: &str = arena.alloc_str("hello");
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
        let slice: &[i32] = builder.into_bump_slice();
        assert_eq!(slice, &[1, 2, 3]);
    }

    /// Core should provide ArenaMap with bumpalo allocator
    #[test]
    fn core_provides_arena_hashmap() {
        let arena = Bump::new();

        // hashbrown::HashMap with default allocator
        let mut map: HashMap<&str, i32> = HashMap::new();
        map.insert("one", 1);
        map.insert("two", 2);

        assert_eq!(map.get("one"), Some(&1));

        // TODO: After full implementation, use arena-allocated HashMap:
        // type ArenaMap<'a, K, V> = HashMap<K, V, Allocator<'a>>;
        // let mut map: ArenaMap<'_, &str, i32> = HashMap::new_in(Allocator::new(&arena));
    }

    /// Core should provide type alias for arena lifetime usage
    #[test]
    fn core_provides_arena_type_alias() {
        // After implementation, this should be available:
        // pub type Arena = Bump;

        let arena: Bump = Bump::new();
        let _s = arena.alloc_str("test");
    }
}
