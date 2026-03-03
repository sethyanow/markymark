//! Typed frontmatter access for downstream consumers.
//!
//! Provides [`FrontmatterValueRef`] (a borrowed value enum), [`FrontmatterMap`]
//! (an ordered map with typed accessors), and the [`TypedFrontmatter`] trait
//! for typed deserialization from frontmatter key-value pairs.

use std::fmt;

use thiserror::Error;

// ── FrontmatterValueRef ────────────────────────────────────────────

/// A borrowed frontmatter value matching the 7-variant shape used by
/// both the parser (`FrontmatterValue<'arena>`) and index
/// (`FrontmatterValueEntry<'arena>`).
///
/// Lives in core so downstream consumers can implement [`TypedFrontmatter`]
/// without depending on parser or index crates.
#[derive(Debug, Clone, PartialEq)]
pub enum FrontmatterValueRef<'a> {
    /// A string value.
    String(&'a str),
    /// An integer value (fits in i64).
    Integer(i64),
    /// A floating-point value (always finite — NaN/inf stored as String upstream).
    Float(f64),
    /// A boolean value.
    Boolean(bool),
    /// A list of typed values.
    List(Vec<FrontmatterValueRef<'a>>),
    /// A map of key-value pairs.
    Map(Vec<(&'a str, FrontmatterValueRef<'a>)>),
    /// An explicit null value.
    Null,
}

impl<'a> FrontmatterValueRef<'a> {
    /// Get as string if this is a `String` variant.
    pub fn as_string(&self) -> Option<&'a str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer if this is an `Integer` variant.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as float if this is a `Float` variant.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as boolean if this is a `Boolean` variant.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as list if this is a `List` variant.
    pub fn as_list(&self) -> Option<&[FrontmatterValueRef<'a>]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }

    /// Get as map entries if this is a `Map` variant.
    pub fn as_map(&self) -> Option<&[(&'a str, FrontmatterValueRef<'a>)]> {
        match self {
            Self::Map(v) => Some(v),
            _ => None,
        }
    }

    /// Returns `true` if this is the `Null` variant.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns the variant name as a static string, useful for error messages.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::String(_) => "String",
            Self::Integer(_) => "Integer",
            Self::Float(_) => "Float",
            Self::Boolean(_) => "Boolean",
            Self::List(_) => "List",
            Self::Map(_) => "Map",
            Self::Null => "Null",
        }
    }
}

// ── FrontmatterMap ─────────────────────────────────────────────────

/// An ordered map of frontmatter key-value pairs with typed accessors.
///
/// Uses linear scan (O(n)) which is appropriate for typical frontmatter
/// with 5–15 fields. First-match semantics for duplicate keys.
#[derive(Debug, Clone)]
pub struct FrontmatterMap<'a> {
    entries: Vec<(&'a str, FrontmatterValueRef<'a>)>,
}

impl<'a> FrontmatterMap<'a> {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Look up a value by key (first match).
    pub fn get(&self, key: &str) -> Option<&FrontmatterValueRef<'a>> {
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Get a string value by key.
    pub fn get_string(&self, key: &str) -> Option<&'a str> {
        self.get(key).and_then(FrontmatterValueRef::as_string)
    }

    /// Get an integer value by key.
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(FrontmatterValueRef::as_integer)
    }

    /// Get a float value by key.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(FrontmatterValueRef::as_float)
    }

    /// Get a boolean value by key.
    pub fn get_boolean(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(FrontmatterValueRef::as_boolean)
    }

    /// Get a list value by key.
    pub fn get_list(&self, key: &str) -> Option<&[FrontmatterValueRef<'a>]> {
        self.get(key).and_then(FrontmatterValueRef::as_list)
    }

    /// Returns `true` if the given key is present and its value is `Null`.
    pub fn is_null(&self, key: &str) -> bool {
        self.get(key).is_some_and(FrontmatterValueRef::is_null)
    }

    /// Iterate over keys.
    pub fn keys(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.entries.iter().map(|(k, _)| *k)
    }

    /// Iterate over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &FrontmatterValueRef<'a>)> {
        self.entries.iter().map(|(k, v)| (*k, v))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for FrontmatterMap<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> From<Vec<(&'a str, FrontmatterValueRef<'a>)>> for FrontmatterMap<'a> {
    fn from(entries: Vec<(&'a str, FrontmatterValueRef<'a>)>) -> Self {
        Self { entries }
    }
}

// ── FrontmatterError ───────────────────────────────────────────────

/// Errors that can occur during typed frontmatter deserialization.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterError {
    /// A required field is missing from the frontmatter.
    #[error("missing required field: {field}")]
    MissingField {
        /// Name of the missing field.
        field: String,
    },

    /// A field has an unexpected type.
    #[error("type mismatch for field `{field}`: expected {expected}, got {actual}")]
    TypeMismatch {
        /// Name of the field.
        field: String,
        /// Expected type name.
        expected: &'static str,
        /// Actual type name.
        actual: String,
    },

    /// A custom deserialization error.
    #[error("{0}")]
    Custom(String),
}

// ── TypedFrontmatter trait ─────────────────────────────────────────

/// Trait for types that can be deserialized from frontmatter key-value pairs.
///
/// Consumers implement this for their domain types. The trait receives a
/// [`FrontmatterMap`] and returns the deserialized value or a
/// [`FrontmatterError`] on the first invalid field (fail-fast).
///
/// # Example
///
/// ```
/// use markymark_core::frontmatter::*;
///
/// struct TaskMeta {
///     title: String,
///     priority: i64,
///     draft: bool,
/// }
///
/// impl TypedFrontmatter for TaskMeta {
///     fn from_frontmatter(map: &FrontmatterMap<'_>) -> Result<Self, FrontmatterError> {
///         let title = map.get_string("title")
///             .ok_or_else(|| FrontmatterError::MissingField { field: "title".into() })?
///             .to_string();
///         let priority = map.get_integer("priority")
///             .ok_or_else(|| FrontmatterError::MissingField { field: "priority".into() })?;
///         let draft = map.get_boolean("draft").unwrap_or(false);
///         Ok(Self { title, priority, draft })
///     }
/// }
/// ```
pub trait TypedFrontmatter: Sized {
    /// Deserialize from a frontmatter map.
    ///
    /// Fails fast on the first missing or mistyped field.
    fn from_frontmatter(map: &FrontmatterMap<'_>) -> Result<Self, FrontmatterError>;
}

// ── Display for FrontmatterValueRef ────────────────────────────────

impl fmt::Display for FrontmatterValueRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s}"),
            Self::Integer(n) => write!(f, "{n}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::List(_) => write!(f, "[list]"),
            Self::Map(_) => write!(f, "{{map}}"),
            Self::Null => write!(f, "null"),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- FrontmatterValueRef accessor tests --

    #[test]
    fn value_ref_string_accessor() {
        let v = FrontmatterValueRef::String("hello");
        assert_eq!(v.as_string(), Some("hello"));
        assert_eq!(v.as_integer(), None);
        assert_eq!(v.as_float(), None);
        assert_eq!(v.as_boolean(), None);
        assert!(v.as_list().is_none());
        assert!(v.as_map().is_none());
        assert!(!v.is_null());
        assert_eq!(v.variant_name(), "String");
    }

    #[test]
    fn value_ref_integer_accessor() {
        let v = FrontmatterValueRef::Integer(42);
        assert_eq!(v.as_integer(), Some(42));
        assert_eq!(v.as_string(), None);
        assert_eq!(v.variant_name(), "Integer");
    }

    #[test]
    fn value_ref_float_accessor() {
        let v = FrontmatterValueRef::Float(3.125);
        assert_eq!(v.as_float(), Some(3.125));
        assert_eq!(v.as_string(), None);
        assert_eq!(v.variant_name(), "Float");
    }

    #[test]
    fn value_ref_boolean_accessor() {
        let v = FrontmatterValueRef::Boolean(true);
        assert_eq!(v.as_boolean(), Some(true));
        assert_eq!(v.as_string(), None);
        assert_eq!(v.variant_name(), "Boolean");
    }

    #[test]
    fn value_ref_list_accessor() {
        let v = FrontmatterValueRef::List(vec![
            FrontmatterValueRef::String("a"),
            FrontmatterValueRef::Integer(1),
        ]);
        let list = v.as_list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].as_string(), Some("a"));
        assert_eq!(list[1].as_integer(), Some(1));
        assert_eq!(v.variant_name(), "List");
    }

    #[test]
    fn value_ref_map_accessor() {
        let v = FrontmatterValueRef::Map(vec![("key", FrontmatterValueRef::String("val"))]);
        let map = v.as_map().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].0, "key");
        assert_eq!(v.variant_name(), "Map");
    }

    #[test]
    fn value_ref_null_accessor() {
        let v = FrontmatterValueRef::Null;
        assert!(v.is_null());
        assert_eq!(v.as_string(), None);
        assert_eq!(v.variant_name(), "Null");
    }

    #[test]
    fn value_ref_nested_list_of_lists() {
        let inner = FrontmatterValueRef::List(vec![FrontmatterValueRef::String("nested")]);
        let outer = FrontmatterValueRef::List(vec![inner]);
        let list = outer.as_list().unwrap();
        assert_eq!(list.len(), 1);
        let inner_list = list[0].as_list().unwrap();
        assert_eq!(inner_list[0].as_string(), Some("nested"));
    }

    #[test]
    fn value_ref_empty_list_is_not_null() {
        let v = FrontmatterValueRef::List(vec![]);
        assert!(!v.is_null());
        assert_eq!(v.as_list().unwrap().len(), 0);
    }

    #[test]
    fn value_ref_empty_map_is_not_null() {
        let v = FrontmatterValueRef::Map(vec![]);
        assert!(!v.is_null());
        assert_eq!(v.as_map().unwrap().len(), 0);
    }

    // -- FrontmatterMap tests --

    fn sample_map<'a>() -> FrontmatterMap<'a> {
        FrontmatterMap::from(vec![
            ("title", FrontmatterValueRef::String("Hello")),
            ("count", FrontmatterValueRef::Integer(5)),
            ("ratio", FrontmatterValueRef::Float(2.5)),
            ("draft", FrontmatterValueRef::Boolean(false)),
            (
                "tags",
                FrontmatterValueRef::List(vec![
                    FrontmatterValueRef::String("rust"),
                    FrontmatterValueRef::String("markdown"),
                ]),
            ),
            ("empty", FrontmatterValueRef::Null),
        ])
    }

    #[test]
    fn map_get_string() {
        let m = sample_map();
        assert_eq!(m.get_string("title"), Some("Hello"));
        assert_eq!(m.get_string("count"), None); // wrong type
        assert_eq!(m.get_string("missing"), None);
    }

    #[test]
    fn map_get_integer() {
        let m = sample_map();
        assert_eq!(m.get_integer("count"), Some(5));
        assert_eq!(m.get_integer("title"), None);
    }

    #[test]
    fn map_get_float() {
        let m = sample_map();
        assert_eq!(m.get_float("ratio"), Some(2.5));
        assert_eq!(m.get_float("count"), None);
    }

    #[test]
    fn map_get_boolean() {
        let m = sample_map();
        assert_eq!(m.get_boolean("draft"), Some(false));
        assert_eq!(m.get_boolean("title"), None);
    }

    #[test]
    fn map_get_list() {
        let m = sample_map();
        let list = m.get_list("tags").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].as_string(), Some("rust"));
    }

    #[test]
    fn map_is_null() {
        let m = sample_map();
        assert!(m.is_null("empty"));
        assert!(!m.is_null("title"));
        assert!(!m.is_null("missing")); // absent key is not null
    }

    #[test]
    fn map_keys_and_len() {
        let m = sample_map();
        assert_eq!(m.len(), 6);
        assert!(!m.is_empty());
        let keys: Vec<_> = m.keys().collect();
        assert_eq!(
            keys,
            vec!["title", "count", "ratio", "draft", "tags", "empty"]
        );
    }

    #[test]
    fn map_empty() {
        let m = FrontmatterMap::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
        assert_eq!(m.get("x"), None);
    }

    #[test]
    fn map_duplicate_keys_first_match() {
        let m = FrontmatterMap::from(vec![
            ("key", FrontmatterValueRef::String("first")),
            ("key", FrontmatterValueRef::String("second")),
        ]);
        assert_eq!(m.get_string("key"), Some("first"));
    }

    // -- FrontmatterError tests --

    #[test]
    fn error_missing_field_display() {
        let err = FrontmatterError::MissingField {
            field: "title".into(),
        };
        assert_eq!(err.to_string(), "missing required field: title");
    }

    #[test]
    fn error_type_mismatch_display() {
        let err = FrontmatterError::TypeMismatch {
            field: "priority".into(),
            expected: "Integer",
            actual: "String".into(),
        };
        assert_eq!(
            err.to_string(),
            "type mismatch for field `priority`: expected Integer, got String"
        );
    }

    #[test]
    fn error_custom_display() {
        let err = FrontmatterError::Custom("bad value".into());
        assert_eq!(err.to_string(), "bad value");
    }

    // -- TypedFrontmatter trait test --

    #[derive(Debug)]
    struct TestMeta {
        title: String,
        priority: i64,
        draft: bool,
    }

    impl TypedFrontmatter for TestMeta {
        fn from_frontmatter(map: &FrontmatterMap<'_>) -> Result<Self, FrontmatterError> {
            let title = map
                .get_string("title")
                .ok_or_else(|| FrontmatterError::MissingField {
                    field: "title".into(),
                })?
                .to_string();

            let priority =
                map.get_integer("priority")
                    .ok_or_else(|| match map.get("priority") {
                        Some(v) => FrontmatterError::TypeMismatch {
                            field: "priority".into(),
                            expected: "Integer",
                            actual: v.variant_name().into(),
                        },
                        None => FrontmatterError::MissingField {
                            field: "priority".into(),
                        },
                    })?;

            let draft = map.get_boolean("draft").unwrap_or(false);

            Ok(Self {
                title,
                priority,
                draft,
            })
        }
    }

    #[test]
    fn typed_frontmatter_success() {
        let map = FrontmatterMap::from(vec![
            ("title", FrontmatterValueRef::String("My Doc")),
            ("priority", FrontmatterValueRef::Integer(2)),
            ("draft", FrontmatterValueRef::Boolean(true)),
        ]);
        let meta = TestMeta::from_frontmatter(&map).unwrap();
        assert_eq!(meta.title, "My Doc");
        assert_eq!(meta.priority, 2);
        assert!(meta.draft);
    }

    #[test]
    fn typed_frontmatter_optional_field() {
        let map = FrontmatterMap::from(vec![
            ("title", FrontmatterValueRef::String("No Draft")),
            ("priority", FrontmatterValueRef::Integer(1)),
        ]);
        let meta = TestMeta::from_frontmatter(&map).unwrap();
        assert!(!meta.draft); // default false
    }

    #[test]
    fn typed_frontmatter_missing_required() {
        let map = FrontmatterMap::from(vec![("priority", FrontmatterValueRef::Integer(1))]);
        let err = TestMeta::from_frontmatter(&map).unwrap_err();
        assert_eq!(
            err,
            FrontmatterError::MissingField {
                field: "title".into()
            }
        );
    }

    #[test]
    fn typed_frontmatter_type_mismatch() {
        let map = FrontmatterMap::from(vec![
            ("title", FrontmatterValueRef::String("Ok")),
            ("priority", FrontmatterValueRef::String("not a number")),
        ]);
        let err = TestMeta::from_frontmatter(&map).unwrap_err();
        assert_eq!(
            err,
            FrontmatterError::TypeMismatch {
                field: "priority".into(),
                expected: "Integer",
                actual: "String".into(),
            }
        );
    }

    #[test]
    fn typed_frontmatter_empty_map_fails() {
        let map = FrontmatterMap::new();
        let err = TestMeta::from_frontmatter(&map).unwrap_err();
        assert!(matches!(err, FrontmatterError::MissingField { .. }));
    }

    // -- Display tests --

    #[test]
    fn value_ref_display() {
        assert_eq!(FrontmatterValueRef::String("hi").to_string(), "hi");
        assert_eq!(FrontmatterValueRef::Integer(42).to_string(), "42");
        assert_eq!(FrontmatterValueRef::Float(1.5).to_string(), "1.5");
        assert_eq!(FrontmatterValueRef::Boolean(true).to_string(), "true");
        assert_eq!(FrontmatterValueRef::List(vec![]).to_string(), "[list]");
        assert_eq!(FrontmatterValueRef::Map(vec![]).to_string(), "{map}");
        assert_eq!(FrontmatterValueRef::Null.to_string(), "null");
    }
}
