//! Metadata and frontmatter: Frontmatter, FrontmatterValue, Properties, PropertyValue.

use markymark_core::arena::ArenaHashMap;

/// Frontmatter
#[derive(Debug, Clone)]
pub struct Frontmatter<'arena> {
    data: ArenaHashMap<'arena, &'arena str, FrontmatterValue<'arena>>,
}

impl<'arena> Frontmatter<'arena> {
    /// Create new frontmatter
    pub(crate) fn new(data: ArenaHashMap<'arena, &'arena str, FrontmatterValue<'arena>>) -> Self {
        Self { data }
    }

    /// Get string value
    pub fn get_string(&self, key: &str) -> Option<&'arena str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    /// Get list value
    pub fn get_list(&self, key: &str) -> Option<Vec<&'arena str>> {
        self.data.get(key).and_then(|v| v.as_list())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FrontmatterValue<'arena> {
    String(&'arena str),
    List(&'arena [&'arena str]),
}

impl<'arena> FrontmatterValue<'arena> {
    fn as_string(&self) -> Option<&'arena str> {
        match self {
            FrontmatterValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<Vec<&'arena str>> {
        match self {
            FrontmatterValue::List(list) => Some(list.to_vec()),
            _ => None,
        }
    }
}

/// Properties (Logseq)
#[derive(Debug, Clone)]
pub struct Properties<'arena> {
    data: ArenaHashMap<'arena, &'arena str, PropertyValue<'arena>>,
}

impl<'arena> Properties<'arena> {
    /// Create new properties
    pub(crate) fn new(data: ArenaHashMap<'arena, &'arena str, PropertyValue<'arena>>) -> Self {
        Self { data }
    }

    /// Get property
    pub fn get(&self, key: &str) -> Option<&PropertyValue<'arena>> {
        self.data.get(key)
    }
}

/// A property value (Logseq)
#[derive(Debug, Clone)]
pub enum PropertyValue<'arena> {
    /// String value
    String(&'arena str),
    /// List of values
    List(&'arena [&'arena str]),
    /// Page reference
    PageRef(&'arena str),
}

impl<'arena> PropertyValue<'arena> {
    /// Get as string, or `None` if this value is a list.
    pub fn as_str(&self) -> Option<&'arena str> {
        match self {
            PropertyValue::String(s) | PropertyValue::PageRef(s) => Some(s),
            PropertyValue::List(_) => None,
        }
    }

    /// Check if this is a list
    pub fn is_list(&self) -> bool {
        matches!(self, PropertyValue::List(_))
    }

    /// Check if this is a page reference
    pub fn is_page_ref(&self) -> bool {
        matches!(self, PropertyValue::PageRef(_))
    }
}
