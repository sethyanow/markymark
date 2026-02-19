//! XML/HTML tag extraction types.

use markymark_core::arena::ArenaHashMap;
use markymark_core::prelude::*;

/// An XML/HTML tag element extracted from markdown
#[derive(Debug)]
pub struct XmlTag<'arena> {
    tag_name: &'arena str,
    attributes: ArenaHashMap<'arena, &'arena str, &'arena str>,
    is_self_closing: bool,
    is_unclosed: bool,
    content: Option<&'arena str>,
    range: Range,
    start_byte: usize,
    end_byte: usize,
}

impl<'arena> XmlTag<'arena> {
    /// Create a new XML tag
    pub(crate) fn new(
        tag_name: &'arena str,
        attributes: ArenaHashMap<'arena, &'arena str, &'arena str>,
        is_self_closing: bool,
        content: Option<&'arena str>,
        range: Range,
        start_byte: usize,
        end_byte: usize,
    ) -> Self {
        Self {
            tag_name,
            attributes,
            is_self_closing,
            is_unclosed: false,
            content,
            range,
            start_byte,
            end_byte,
        }
    }

    /// Create an unclosed XML tag (opening tag with no matching close)
    pub(crate) fn unclosed(
        tag_name: &'arena str,
        attributes: ArenaHashMap<'arena, &'arena str, &'arena str>,
        range: Range,
        start_byte: usize,
        end_byte: usize,
    ) -> Self {
        Self {
            tag_name,
            attributes,
            is_self_closing: false,
            is_unclosed: true,
            content: None,
            range,
            start_byte,
            end_byte,
        }
    }

    /// Get tag name (e.g. "div", "agent", "br")
    pub fn tag_name(&self) -> &'arena str {
        self.tag_name
    }

    /// Get attributes as key-value pairs
    pub fn attributes(&self) -> &ArenaHashMap<'arena, &'arena str, &'arena str> {
        &self.attributes
    }

    /// Whether this is a self-closing tag (e.g. `<br/>`, `<img ...>`)
    pub fn is_self_closing(&self) -> bool {
        self.is_self_closing
    }

    /// Whether this tag has no matching closing tag
    pub fn is_unclosed(&self) -> bool {
        self.is_unclosed
    }

    /// Text content between opening and closing tags, if applicable
    pub fn content(&self) -> Option<&'arena str> {
        self.content
    }

    /// Get range in source document
    pub fn range(&self) -> Range {
        self.range
    }

    /// Get byte range as (start_byte, end_byte)
    pub fn byte_range(&self) -> (usize, usize) {
        (self.start_byte, self.end_byte)
    }
}
