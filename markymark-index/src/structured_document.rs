//! Index for structured (non-markdown) documents.
//!
//! Wraps a [`StructuredAst`] and provides key-path lookups, outline generation,
//! and symbol search for JSON, YAML, TOML, and other structured formats.

use std::collections::HashMap;

use markymark_core::structured::{DocumentKind, KeyEntry, StructuredAst, ValueKind};
use markymark_core::Range;

/// Index for a single structured document.
///
/// Provides efficient key-path lookups and outline generation from
/// the flat [`KeyEntry`] list produced by structured parsers.
pub struct StructuredDocumentIndex {
    ast: StructuredAst,
    /// key path → index into `ast.keys` for O(1) lookup.
    path_to_idx: HashMap<String, usize>,
}

impl StructuredDocumentIndex {
    /// Create a new index from a structured AST.
    pub fn from_ast(ast: StructuredAst) -> Self {
        let path_to_idx = ast
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.path.clone(), i))
            .collect();
        Self { ast, path_to_idx }
    }

    /// The document kind (Json, Yaml, Toml, etc.).
    pub fn kind(&self) -> DocumentKind {
        self.ast.kind
    }

    /// All key entries in the document.
    pub fn keys(&self) -> &[KeyEntry] {
        &self.ast.keys
    }

    /// Root-level key entries (depth == 0).
    pub fn root_keys(&self) -> Vec<&KeyEntry> {
        self.ast.root_keys()
    }

    /// Look up a key entry by its full path (e.g. "database.host").
    pub fn key_by_path(&self, path: &str) -> Option<&KeyEntry> {
        self.path_to_idx
            .get(path)
            .and_then(|&idx| self.ast.keys.get(idx))
    }

    /// Find the key entry whose key range contains the cursor position.
    pub fn find_key_at_position(&self, cursor: markymark_core::Position) -> Option<&KeyEntry> {
        self.ast.keys.iter().find(|k| k.key_range.contains(cursor))
    }

    /// Number of key entries in the document.
    pub fn key_count(&self) -> usize {
        self.ast.keys.len()
    }

    /// Search key paths matching a query (case-insensitive substring match).
    pub fn search_keys(&self, query: &str) -> Vec<&KeyEntry> {
        let query_lower = query.to_lowercase();
        self.ast
            .keys
            .iter()
            .filter(|k| {
                k.path.to_lowercase().contains(&query_lower)
                    || k.key.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get keys that are direct children of the given parent path.
    pub fn children_of(&self, parent_path: &str) -> Vec<&KeyEntry> {
        let parent_depth = if parent_path.is_empty() {
            0
        } else {
            // Find the parent entry to get its depth
            self.key_by_path(parent_path)
                .map(|k| k.depth + 1)
                .unwrap_or(0)
        };

        let prefix = if parent_path.is_empty() {
            String::new()
        } else {
            format!("{parent_path}.")
        };

        self.ast
            .keys
            .iter()
            .filter(|k| {
                k.depth == parent_depth && (parent_path.is_empty() || k.path.starts_with(&prefix))
            })
            .collect()
    }

    /// Generate a flat list of all key paths with their ranges,
    /// suitable for search-symbols and export-index.
    pub fn key_paths_with_ranges(&self) -> Vec<(&str, &str, ValueKind, Range)> {
        self.ast
            .keys
            .iter()
            .map(|k| (k.path.as_str(), k.key.as_str(), k.value_kind, k.key_range))
            .collect()
    }
}

impl std::fmt::Debug for StructuredDocumentIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructuredDocumentIndex")
            .field("kind", &self.ast.kind)
            .field("key_count", &self.ast.keys.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ast(kind: DocumentKind, keys: Vec<KeyEntry>) -> StructuredAst {
        StructuredAst {
            source: String::new(),
            kind,
            keys,
        }
    }

    fn key(path: &str, key_name: &str, depth: usize, vk: ValueKind) -> KeyEntry {
        KeyEntry {
            path: path.to_string(),
            key: key_name.to_string(),
            depth,
            value_kind: vk,
            key_range: Range::new(
                markymark_core::Position::new(0, 0),
                markymark_core::Position::new(0, 0),
            ),
            value_range: Range::new(
                markymark_core::Position::new(0, 0),
                markymark_core::Position::new(0, 0),
            ),
        }
    }

    fn key_with_range(
        path: &str,
        key_name: &str,
        depth: usize,
        vk: ValueKind,
        key_range: Range,
    ) -> KeyEntry {
        KeyEntry {
            path: path.to_string(),
            key: key_name.to_string(),
            depth,
            value_kind: vk,
            key_range,
            value_range: key_range,
        }
    }

    #[test]
    fn test_from_ast_and_kind() {
        let ast = make_ast(DocumentKind::Json, vec![]);
        let idx = StructuredDocumentIndex::from_ast(ast);
        assert_eq!(idx.kind(), DocumentKind::Json);
        assert_eq!(idx.key_count(), 0);
    }

    #[test]
    fn test_key_by_path() {
        let ast = make_ast(
            DocumentKind::Yaml,
            vec![
                key("db", "db", 0, ValueKind::Object),
                key("db.host", "host", 1, ValueKind::String),
                key("db.port", "port", 1, ValueKind::Number),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        assert!(idx.key_by_path("db.host").is_some());
        assert_eq!(
            idx.key_by_path("db.host").unwrap().value_kind,
            ValueKind::String
        );
        assert!(idx.key_by_path("nonexistent").is_none());
    }

    #[test]
    fn test_root_keys() {
        let ast = make_ast(
            DocumentKind::Toml,
            vec![
                key("a", "a", 0, ValueKind::Object),
                key("a.x", "x", 1, ValueKind::String),
                key("b", "b", 0, ValueKind::Number),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        let roots = idx.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].key, "a");
        assert_eq!(roots[1].key, "b");
    }

    #[test]
    fn test_search_keys() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![
                key("database", "database", 0, ValueKind::Object),
                key("database.host", "host", 1, ValueKind::String),
                key("logging", "logging", 0, ValueKind::Object),
                key("logging.level", "level", 1, ValueKind::String),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        let results = idx.search_keys("host");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "database.host");

        // Case-insensitive
        let results = idx.search_keys("DATABASE");
        assert_eq!(results.len(), 2); // "database" and "database.host"
    }

    #[test]
    fn test_children_of() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![
                key("db", "db", 0, ValueKind::Object),
                key("db.host", "host", 1, ValueKind::String),
                key("db.port", "port", 1, ValueKind::Number),
                key("log", "log", 0, ValueKind::Object),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        let children = idx.children_of("db");
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].key, "host");
        assert_eq!(children[1].key, "port");

        // Root children
        let roots = idx.children_of("");
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn test_key_paths_with_ranges() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![
                key("a", "a", 0, ValueKind::String),
                key("b", "b", 0, ValueKind::Number),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        let paths = idx.key_paths_with_ranges();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].0, "a");
        assert_eq!(paths[1].0, "b");
    }

    #[test]
    fn test_find_key_at_position_hit() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![
                key_with_range(
                    "database",
                    "database",
                    0,
                    ValueKind::Object,
                    Range::new(
                        markymark_core::Position::new(1, 2),
                        markymark_core::Position::new(1, 10),
                    ),
                ),
                key_with_range(
                    "database.host",
                    "host",
                    1,
                    ValueKind::String,
                    Range::new(
                        markymark_core::Position::new(2, 4),
                        markymark_core::Position::new(2, 8),
                    ),
                ),
            ],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        let hit = idx
            .find_key_at_position(markymark_core::Position::new(2, 5))
            .map(|k| k.path.clone());
        assert_eq!(hit.as_deref(), Some("database.host"));
    }

    #[test]
    fn test_find_key_at_position_miss() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![key_with_range(
                "database.host",
                "host",
                1,
                ValueKind::String,
                Range::new(
                    markymark_core::Position::new(2, 4),
                    markymark_core::Position::new(2, 8),
                ),
            )],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        assert!(idx
            .find_key_at_position(markymark_core::Position::new(2, 20))
            .is_none());
    }

    #[test]
    fn test_find_key_at_position_boundary() {
        let ast = make_ast(
            DocumentKind::Json,
            vec![key_with_range(
                "database.host",
                "host",
                1,
                ValueKind::String,
                Range::new(
                    markymark_core::Position::new(2, 4),
                    markymark_core::Position::new(2, 8),
                ),
            )],
        );
        let idx = StructuredDocumentIndex::from_ast(ast);

        assert!(
            idx.find_key_at_position(markymark_core::Position::new(2, 4))
                .is_some(),
            "range start must be inclusive"
        );
        assert!(
            idx.find_key_at_position(markymark_core::Position::new(2, 8))
                .is_none(),
            "range end must be exclusive"
        );
    }
}
