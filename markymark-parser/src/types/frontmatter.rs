//! Metadata and frontmatter: Frontmatter, FrontmatterValue, Properties, PropertyValue.

use markymark_core::arena::ArenaHashMap;
use markymark_core::frontmatter::{FrontmatterMap, FrontmatterValueRef};

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

    /// Get string value for the given key.
    pub fn get_string(&self, key: &str) -> Option<&'arena str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    /// Get integer value for the given key.
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        self.data.get(key).and_then(|v| v.as_integer())
    }

    /// Get float value for the given key.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_float())
    }

    /// Get boolean value for the given key.
    pub fn get_boolean(&self, key: &str) -> Option<bool> {
        self.data.get(key).and_then(|v| v.as_boolean())
    }

    /// Get string list value for the given key (string items only).
    pub fn get_string_list(&self, key: &str) -> Option<Vec<&'arena str>> {
        self.data.get(key).and_then(|v| v.as_string_list())
    }

    /// Iterate over all key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&'arena str, &FrontmatterValue<'arena>)> {
        self.data.iter().map(|(k, v)| (*k, v))
    }
}

/// A single frontmatter value.
#[derive(Debug, Clone)]
pub enum FrontmatterValue<'arena> {
    /// A simple string value.
    String(&'arena str),
    /// An integer value (fits in i64).
    Integer(i64),
    /// A floating-point value (always finite — NaN/inf stored as String).
    Float(f64),
    /// A boolean value.
    Boolean(bool),
    /// A list of typed values.
    List(&'arena [FrontmatterValue<'arena>]),
    /// A map of key-value pairs (for programmatic construction and future multi-line support).
    Map(&'arena [(&'arena str, FrontmatterValue<'arena>)]),
    /// An explicit null value (empty, `null`, `~`).
    Null,
}

impl<'arena> FrontmatterValue<'arena> {
    /// Get as string if this is a String variant.
    pub fn as_string(&self) -> Option<&'arena str> {
        match self {
            FrontmatterValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as string list if this is a List of Strings.
    ///
    /// Returns only the String items from the list, skipping non-string values.
    pub fn as_string_list(&self) -> Option<Vec<&'arena str>> {
        match self {
            FrontmatterValue::List(list) => {
                let strings: Vec<&str> = list.iter().filter_map(|v| v.as_string()).collect();
                Some(strings)
            }
            _ => None,
        }
    }

    /// Get as integer if this is an Integer variant.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            FrontmatterValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as float if this is a Float variant.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            FrontmatterValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as boolean if this is a Boolean variant.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            FrontmatterValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Check if this is a Null variant.
    pub fn is_null(&self) -> bool {
        matches!(self, FrontmatterValue::Null)
    }

    /// Coerce any variant to a string representation.
    pub fn to_string_lossy(&self) -> String {
        match self {
            FrontmatterValue::String(s) => (*s).to_string(),
            FrontmatterValue::Integer(n) => n.to_string(),
            FrontmatterValue::Float(f) => f.to_string(),
            FrontmatterValue::Boolean(b) => b.to_string(),
            FrontmatterValue::List(_) => "[list]".to_string(),
            FrontmatterValue::Map(_) => "{map}".to_string(),
            FrontmatterValue::Null => String::new(),
        }
    }
}

// ── Conversions to core FrontmatterValueRef / FrontmatterMap ───────

impl<'a> From<&FrontmatterValue<'a>> for FrontmatterValueRef<'a> {
    fn from(value: &FrontmatterValue<'a>) -> Self {
        match value {
            FrontmatterValue::String(s) => FrontmatterValueRef::String(s),
            FrontmatterValue::Integer(n) => FrontmatterValueRef::Integer(*n),
            FrontmatterValue::Float(f) => {
                debug_assert!(f.is_finite(), "FrontmatterValue::Float must be finite");
                FrontmatterValueRef::Float(*f)
            }
            FrontmatterValue::Boolean(b) => FrontmatterValueRef::Boolean(*b),
            FrontmatterValue::List(items) => {
                FrontmatterValueRef::List(items.iter().map(|v| v.into()).collect())
            }
            FrontmatterValue::Map(entries) => {
                FrontmatterValueRef::Map(entries.iter().map(|(k, v)| (*k, v.into())).collect())
            }
            FrontmatterValue::Null => FrontmatterValueRef::Null,
        }
    }
}

impl<'a> From<&Frontmatter<'a>> for FrontmatterMap<'a> {
    fn from(fm: &Frontmatter<'a>) -> Self {
        let entries: Vec<(&'a str, FrontmatterValueRef<'a>)> =
            fm.iter().map(|(k, v)| (k, v.into())).collect();
        FrontmatterMap::from(entries)
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

    /// Iterate over all key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&'arena str, &PropertyValue<'arena>)> {
        self.data.iter().map(|(k, v)| (*k, v))
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
