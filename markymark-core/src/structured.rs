//! Types for structured (non-markdown) document support.
//!
//! These types model key-value documents such as JSON, YAML, TOML, .env, and .ini files.
//! Each format is parsed into a uniform [`StructuredAst`] containing [`KeyEntry`] items
//! with byte-accurate source ranges suitable for LSP integration.

use std::fmt;
use std::path::Path;

use crate::Range;

/// The kind of document, determined by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    /// Markdown (`.md`, `.markdown`).
    Markdown,
    /// JSON (`.json`).
    Json,
    /// JSON with comments (`.jsonc`).
    JsonC,
    /// JSON5 (`.json5`).
    Json5,
    /// JSON Lines (`.jsonl`).
    JsonLines,
    /// YAML (`.yaml`, `.yml`).
    Yaml,
    /// TOML (`.toml`).
    Toml,
    /// Dotenv (`.env`, bare `.env` dotfile).
    DotEnv,
    /// INI / config (`.ini`, `.cfg`).
    Ini,
}

impl DocumentKind {
    /// Determine the document kind from a file path.
    ///
    /// Returns `None` for unsupported or unrecognised extensions.
    /// Handles case-insensitive matching and the bare `.env` dotfile edge case
    /// where [`Path::extension`] returns `None`.
    pub fn from_path(path: &Path) -> Option<Self> {
        // Try standard extension first.
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lower = ext.to_ascii_lowercase();
            return match lower.as_str() {
                "md" | "markdown" | "mdx" => Some(Self::Markdown),
                "json" => Some(Self::Json),
                "jsonc" => Some(Self::JsonC),
                "json5" => Some(Self::Json5),
                "jsonl" => Some(Self::JsonLines),
                "yaml" | "yml" => Some(Self::Yaml),
                "toml" => Some(Self::Toml),
                "env" => Some(Self::DotEnv),
                "ini" | "cfg" => Some(Self::Ini),
                _ => None,
            };
        }

        // Fallback: check bare dotfile names (e.g. `.env` has no extension in Rust).
        let file_name = path.file_name().and_then(|n| n.to_str())?;
        match file_name {
            ".env" => Some(Self::DotEnv),
            _ => None,
        }
    }

    /// Return the file extensions associated with this document kind.
    pub fn extensions(&self) -> &[&str] {
        match self {
            Self::Markdown => &["md", "markdown", "mdx"],
            Self::Json => &["json"],
            Self::JsonC => &["jsonc"],
            Self::Json5 => &["json5"],
            Self::JsonLines => &["jsonl"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::DotEnv => &["env"],
            Self::Ini => &["ini", "cfg"],
        }
    }
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markdown => write!(f, "Markdown"),
            Self::Json => write!(f, "JSON"),
            Self::JsonC => write!(f, "JSON with Comments"),
            Self::Json5 => write!(f, "JSON5"),
            Self::JsonLines => write!(f, "JSON Lines"),
            Self::Yaml => write!(f, "YAML"),
            Self::Toml => write!(f, "TOML"),
            Self::DotEnv => write!(f, "dotenv"),
            Self::Ini => write!(f, "INI"),
        }
    }
}

/// The kind of a value in a structured document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    /// A string value.
    String,
    /// A numeric value.
    Number,
    /// A boolean value.
    Boolean,
    /// A null / nil value.
    Null,
    /// An array / sequence.
    Array,
    /// An object / mapping / table.
    Object,
}

/// A single key entry extracted from a structured document.
///
/// Represents one key at a specific nesting depth, with its full dotted path,
/// value classification, and byte-accurate source ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEntry {
    /// Full dotted path (e.g. `"database.host"`, `"servers[0].port"`).
    pub path: String,
    /// Leaf key name (e.g. `"host"`, `"port"`).
    pub key: String,
    /// Nesting depth (0 = top-level).
    pub depth: usize,
    /// Classification of the value.
    pub value_kind: ValueKind,
    /// Source range of the key in the document.
    pub key_range: Range,
    /// Source range of the value in the document.
    pub value_range: Range,
}

/// A parsed structured document.
///
/// This is the uniform output produced by all non-markdown format parsers.
/// It holds the original source, the document kind, and the extracted key entries.
#[derive(Debug, Clone)]
pub struct StructuredAst {
    /// Original source text.
    pub source: String,
    /// Document kind.
    pub kind: DocumentKind,
    /// Extracted key entries with full paths and source ranges.
    pub keys: Vec<KeyEntry>,
}

impl StructuredAst {
    /// Return only the top-level (depth == 0) key entries.
    pub fn root_keys(&self) -> Vec<&KeyEntry> {
        self.keys.iter().filter(|k| k.depth == 0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;
    use std::path::Path;

    // --- DocumentKind::from_path tests ---

    #[test]
    fn test_document_kind_json() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.json")),
            Some(DocumentKind::Json)
        );
    }

    #[test]
    fn test_document_kind_jsonc() {
        assert_eq!(
            DocumentKind::from_path(Path::new("tsconfig.jsonc")),
            Some(DocumentKind::JsonC)
        );
    }

    #[test]
    fn test_document_kind_json5() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.json5")),
            Some(DocumentKind::Json5)
        );
    }

    #[test]
    fn test_document_kind_jsonl() {
        assert_eq!(
            DocumentKind::from_path(Path::new("logs.jsonl")),
            Some(DocumentKind::JsonLines)
        );
    }

    #[test]
    fn test_document_kind_yaml() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.yaml")),
            Some(DocumentKind::Yaml)
        );
    }

    #[test]
    fn test_document_kind_yml() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.yml")),
            Some(DocumentKind::Yaml)
        );
    }

    #[test]
    fn test_document_kind_toml() {
        assert_eq!(
            DocumentKind::from_path(Path::new("Cargo.toml")),
            Some(DocumentKind::Toml)
        );
    }

    #[test]
    fn test_document_kind_env() {
        // CRITICAL: Path::new(".env").extension() returns None.
        // Must fall back to file_name() check.
        assert_eq!(
            DocumentKind::from_path(Path::new(".env")),
            Some(DocumentKind::DotEnv)
        );
    }

    #[test]
    fn test_document_kind_env_named() {
        assert_eq!(
            DocumentKind::from_path(Path::new("prod.env")),
            Some(DocumentKind::DotEnv)
        );
    }

    #[test]
    fn test_document_kind_env_local() {
        // .env.local has extension "local", not "env" — should NOT match.
        assert_eq!(DocumentKind::from_path(Path::new(".env.local")), None);
    }

    #[test]
    fn test_document_kind_ini() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.ini")),
            Some(DocumentKind::Ini)
        );
    }

    #[test]
    fn test_document_kind_cfg() {
        assert_eq!(
            DocumentKind::from_path(Path::new("setup.cfg")),
            Some(DocumentKind::Ini)
        );
    }

    #[test]
    fn test_document_kind_markdown() {
        assert_eq!(
            DocumentKind::from_path(Path::new("README.md")),
            Some(DocumentKind::Markdown)
        );
    }

    #[test]
    fn test_document_kind_markdown_long() {
        assert_eq!(
            DocumentKind::from_path(Path::new("doc.markdown")),
            Some(DocumentKind::Markdown)
        );
    }

    #[test]
    fn test_document_kind_unsupported() {
        assert_eq!(DocumentKind::from_path(Path::new("main.rs")), None);
    }

    #[test]
    fn test_document_kind_no_extension() {
        assert_eq!(DocumentKind::from_path(Path::new("Makefile")), None);
    }

    #[test]
    fn test_document_kind_case_insensitive() {
        assert_eq!(
            DocumentKind::from_path(Path::new("config.JSON")),
            Some(DocumentKind::Json)
        );
        assert_eq!(
            DocumentKind::from_path(Path::new("config.Yaml")),
            Some(DocumentKind::Yaml)
        );
        assert_eq!(
            DocumentKind::from_path(Path::new("config.TOML")),
            Some(DocumentKind::Toml)
        );
    }

    // --- DocumentKind::extensions tests ---

    #[test]
    fn test_extensions_yaml_has_both() {
        let exts = DocumentKind::Yaml.extensions();
        assert!(exts.contains(&"yaml"));
        assert!(exts.contains(&"yml"));
    }

    #[test]
    fn test_extensions_ini_has_both() {
        let exts = DocumentKind::Ini.extensions();
        assert!(exts.contains(&"ini"));
        assert!(exts.contains(&"cfg"));
    }

    // --- Display tests ---

    #[test]
    fn test_display_json() {
        assert_eq!(DocumentKind::Json.to_string(), "JSON");
    }

    #[test]
    fn test_display_yaml() {
        assert_eq!(DocumentKind::Yaml.to_string(), "YAML");
    }

    // --- KeyEntry and StructuredAst tests ---

    fn make_range(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
        Range::new(Position::new(sl, sc), Position::new(el, ec))
    }

    #[test]
    fn test_key_entry_construction() {
        let entry = KeyEntry {
            path: "database.host".to_string(),
            key: "host".to_string(),
            depth: 1,
            value_kind: ValueKind::String,
            key_range: make_range(2, 4, 2, 8),
            value_range: make_range(2, 10, 2, 25),
        };
        assert_eq!(entry.path, "database.host");
        assert_eq!(entry.depth, 1);
        assert_eq!(entry.value_kind, ValueKind::String);
    }

    #[test]
    fn test_structured_ast_root_keys() {
        let ast = StructuredAst {
            source: String::new(),
            kind: DocumentKind::Json,
            keys: vec![
                KeyEntry {
                    path: "name".to_string(),
                    key: "name".to_string(),
                    depth: 0,
                    value_kind: ValueKind::String,
                    key_range: make_range(0, 0, 0, 4),
                    value_range: make_range(0, 6, 0, 12),
                },
                KeyEntry {
                    path: "database.host".to_string(),
                    key: "host".to_string(),
                    depth: 1,
                    value_kind: ValueKind::String,
                    key_range: make_range(2, 4, 2, 8),
                    value_range: make_range(2, 10, 2, 25),
                },
                KeyEntry {
                    path: "version".to_string(),
                    key: "version".to_string(),
                    depth: 0,
                    value_kind: ValueKind::Number,
                    key_range: make_range(4, 0, 4, 7),
                    value_range: make_range(4, 9, 4, 10),
                },
            ],
        };
        let roots = ast.root_keys();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].key, "name");
        assert_eq!(roots[1].key, "version");
    }
}
