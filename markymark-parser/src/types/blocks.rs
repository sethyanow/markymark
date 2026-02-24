//! List and task structures: BlockId, BlockRef, Tag, Embed, Task, TaskState, Callout, QueryBlock.

use markymark_core::prelude::*;

/// Block ID (Obsidian)
#[derive(Debug, Clone)]
pub struct BlockId<'arena> {
    id: &'arena str,
    /// Source range of the block ID occurrence (covers `^block-id` in source).
    range: Range,
    /// Byte offset of the `^` character.
    start_byte: usize,
    /// Byte offset one past the last character of the block ID.
    end_byte: usize,
}

impl<'arena> BlockId<'arena> {
    /// Create a new block ID with its source range and byte offsets.
    #[cfg(test)]
    pub(crate) fn new(id: &'arena str, range: Range, start_byte: usize, end_byte: usize) -> Self {
        Self {
            id,
            range,
            start_byte,
            end_byte,
        }
    }

    /// Get ID
    pub fn id(&self) -> &'arena str {
        self.id
    }

    /// Get the source range of this block ID.
    pub fn range(&self) -> Range {
        self.range
    }

    /// Byte offset of the `^` character.
    pub fn start_byte(&self) -> usize {
        self.start_byte
    }

    /// Byte offset one past the last character of the block ID.
    pub fn end_byte(&self) -> usize {
        self.end_byte
    }
}

/// Block reference (Logseq)
#[derive(Debug, Clone)]
pub struct BlockRef<'arena> {
    uuid: &'arena str,
    /// Source range covering the full `((uuid))` pattern.
    range: Range,
}

impl<'arena> BlockRef<'arena> {
    /// Create a new block reference
    #[cfg(test)]
    pub(crate) fn new(uuid: &'arena str, range: Range) -> Self {
        Self { uuid, range }
    }

    /// Get UUID
    pub fn uuid(&self) -> &'arena str {
        self.uuid
    }

    /// Get source range of the `((uuid))` pattern.
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A tag
#[derive(Debug, Clone)]
pub struct Tag<'arena> {
    name: &'arena str,
}

impl<'arena> Tag<'arena> {
    /// Create a new tag
    #[cfg(test)]
    pub(crate) fn new(name: &'arena str) -> Self {
        Self { name }
    }

    /// Get tag name
    pub fn name(&self) -> &'arena str {
        self.name
    }

    /// Get tag segments (for nested tags like #project/feature/bug)
    pub fn segments(&self) -> Vec<&'arena str> {
        self.name.split('/').collect()
    }
}

/// An embed
#[derive(Debug, Clone)]
pub struct Embed<'arena> {
    target: &'arena str,
}

impl<'arena> Embed<'arena> {
    /// Create a new embed
    #[cfg(test)]
    pub(crate) fn new(target: &'arena str) -> Self {
        Self { target }
    }

    /// Get target
    pub fn target(&self) -> &'arena str {
        self.target
    }

    /// Check if this is an embed
    pub fn is_embed(&self) -> bool {
        true
    }
}

/// A task
#[derive(Debug, Clone)]
pub struct Task<'arena> {
    state: TaskState<'arena>,
}

impl<'arena> Task<'arena> {
    /// Create a new task
    #[cfg(test)]
    pub(crate) fn new(state: TaskState<'arena>) -> Self {
        Self { state }
    }

    /// Get task state
    pub fn state(&self) -> &TaskState<'arena> {
        &self.state
    }
}

/// Task state
#[derive(Debug, Clone)]
pub struct TaskState<'arena> {
    name: &'arena str,
}

impl<'arena> TaskState<'arena> {
    /// Create a new task state
    #[cfg(test)]
    pub(crate) fn new(name: &'arena str) -> Self {
        Self { name }
    }

    /// Get state as string
    pub fn as_str(&self) -> &'arena str {
        self.name
    }
}

/// Callout (Obsidian)
#[derive(Debug, Clone)]
pub struct Callout<'arena> {
    callout_type: &'arena str,
    title: Option<&'arena str>,
}

impl<'arena> Callout<'arena> {
    /// Create a new callout
    #[cfg(test)]
    pub(crate) fn new(callout_type: &'arena str, title: Option<&'arena str>) -> Self {
        Self {
            callout_type,
            title,
        }
    }

    /// Get callout type
    pub fn callout_type(&self) -> &'arena str {
        self.callout_type
    }

    /// Get title
    pub fn title(&self) -> Option<&'arena str> {
        self.title
    }
}

/// Query block (Logseq)
#[derive(Debug, Clone)]
pub struct QueryBlock<'arena> {
    query: &'arena str,
}

impl<'arena> QueryBlock<'arena> {
    /// Create a new query block
    #[cfg(test)]
    pub(crate) fn new(query: &'arena str) -> Self {
        Self { query }
    }

    /// Get query text
    pub fn query_text(&self) -> &'arena str {
        self.query
    }
}
