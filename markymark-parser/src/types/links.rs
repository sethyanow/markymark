//! Link types: WikiLink, MarkdownLink, LinkDefinition.

use markymark_core::prelude::*;

/// A wiki link
#[derive(Debug, Clone)]
pub struct WikiLink<'arena> {
    target: &'arena str,
    alias: Option<&'arena str>,
    heading: Option<&'arena str>,
    block_id: Option<&'arena str>,
    range: Range,
}

impl<'arena> WikiLink<'arena> {
    /// Create a new wiki link
    pub(crate) fn new(
        target: &'arena str,
        alias: Option<&'arena str>,
        heading: Option<&'arena str>,
        block_id: Option<&'arena str>,
        range: Range,
    ) -> Self {
        Self {
            target,
            alias,
            heading,
            block_id,
            range,
        }
    }

    /// Get target page
    pub fn target_page(&self) -> Option<&'arena str> {
        if self.target.is_empty() {
            None
        } else {
            Some(self.target)
        }
    }

    /// Get alias
    pub fn alias(&self) -> Option<&'arena str> {
        self.alias
    }

    /// Get target heading
    pub fn target_heading(&self) -> Option<&'arena str> {
        self.heading
    }

    /// Get target block ID
    pub fn target_block_id(&self) -> Option<&'arena str> {
        self.block_id
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A markdown link
#[derive(Debug, Clone)]
pub struct MarkdownLink<'arena> {
    text: &'arena str,
    url: &'arena str,
    anchor: Option<&'arena str>,
    reference: Option<&'arena str>,
    range: Range,
}

impl<'arena> MarkdownLink<'arena> {
    /// Create a new markdown link
    pub(crate) fn new(
        text: &'arena str,
        url: &'arena str,
        anchor: Option<&'arena str>,
        reference: Option<&'arena str>,
        range: Range,
    ) -> Self {
        Self {
            text,
            url,
            anchor,
            reference,
            range,
        }
    }

    /// Get link text
    pub fn text(&self) -> &'arena str {
        self.text
    }

    /// Get URL
    pub fn url(&self) -> &'arena str {
        self.url
    }

    /// Get anchor
    pub fn anchor(&self) -> Option<&'arena str> {
        self.anchor
    }

    /// Get reference
    pub fn reference(&self) -> Option<&'arena str> {
        self.reference
    }

    /// Get range
    pub fn range(&self) -> Range {
        self.range
    }
}

/// A link definition
#[derive(Debug, Clone)]
pub struct LinkDefinition<'arena> {
    label: &'arena str,
    url: &'arena str,
    title: Option<&'arena str>,
}

impl<'arena> LinkDefinition<'arena> {
    /// Create a new link definition
    pub(crate) fn new(label: &'arena str, url: &'arena str, title: Option<&'arena str>) -> Self {
        Self { label, url, title }
    }

    /// Get label
    pub fn label(&self) -> &'arena str {
        self.label
    }

    /// Get URL
    pub fn url(&self) -> &'arena str {
        self.url
    }

    /// Get title
    pub fn title(&self) -> Option<&'arena str> {
        self.title
    }
}
