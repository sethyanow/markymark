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
