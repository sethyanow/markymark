//! Parsers for structured (non-markdown) document formats.
//!
//! Each format has its own submodule that produces a [`StructuredAst`] with
//! byte-accurate source ranges. The [`parse_structured`] function dispatches
//! to the appropriate parser based on [`DocumentKind`].

mod flat;
pub(crate) mod json;
mod json5;
mod jsonl;
mod toml;
mod yaml;

use markymark_core::structured::{DocumentKind, StructuredAst};
use markymark_core::CoreError;

/// Parse a structured document into a [`StructuredAst`].
///
/// Returns an error for [`DocumentKind::Markdown`] (use the main parser instead)
/// or for formats not yet implemented.
pub fn parse_structured(source: &str, kind: DocumentKind) -> Result<StructuredAst, CoreError> {
    match kind {
        DocumentKind::Markdown => Err(CoreError::Message(
            "use the markdown parser for Markdown documents".to_string(),
        )),
        DocumentKind::Json => json::parse_json(source),
        DocumentKind::JsonC => json::parse_json(source), // Verified (marky-lkj.13): tree-sitter-json 0.24 tolerates //, /* */, and trailing commas
        DocumentKind::Json5 => json5::parse_json5(source),
        DocumentKind::JsonLines => jsonl::parse_jsonl(source),
        DocumentKind::Yaml => yaml::parse_yaml(source),
        DocumentKind::Toml => toml::parse_toml(source),
        DocumentKind::DotEnv => flat::parse_flat(source, kind),
        DocumentKind::Ini => flat::parse_flat(source, kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_structured_dispatch_json() {
        let result = parse_structured(r#"{"key": "val"}"#, DocumentKind::Json);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::Json);
        assert_eq!(ast.keys.len(), 1);
    }

    #[test]
    fn test_parse_structured_dispatch_markdown_errors() {
        let result = parse_structured("# Hello", DocumentKind::Markdown);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("markdown parser"));
    }

    #[test]
    fn test_parse_structured_dispatch_yaml() {
        let result = parse_structured("key: value", DocumentKind::Yaml);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::Yaml);
        assert_eq!(ast.keys.len(), 1);
    }

    #[test]
    fn test_parse_structured_dispatch_toml() {
        let result = parse_structured("key = \"value\"", DocumentKind::Toml);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::Toml);
        assert_eq!(ast.keys.len(), 1);
    }

    #[test]
    fn test_parse_structured_dispatch_jsonl() {
        let result = parse_structured("{\"a\": 1}\n{\"b\": 2}", DocumentKind::JsonLines);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::JsonLines);
        assert!(ast.keys.len() >= 2); // at least 2 line entries
    }

    #[test]
    fn test_parse_structured_dispatch_jsonc() {
        // JSONC is handled by the JSON parser (tree-sitter-json tolerates comments)
        let result = parse_structured(r#"{"key": "val"}"#, DocumentKind::JsonC);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_structured_dispatch_dotenv() {
        let result = parse_structured("KEY=value", DocumentKind::DotEnv);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::DotEnv);
        assert_eq!(ast.keys.len(), 1);
    }

    #[test]
    fn test_parse_structured_dispatch_ini() {
        let result = parse_structured("[section]\nkey = value", DocumentKind::Ini);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::Ini);
        assert_eq!(ast.keys.len(), 2); // section + key
    }

    #[test]
    fn test_parse_structured_dispatch_json5() {
        let result = parse_structured("{key: 'val'}", DocumentKind::Json5);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert_eq!(ast.kind, DocumentKind::Json5);
        assert_eq!(ast.keys.len(), 1);
        assert_eq!(ast.keys[0].key, "key");
    }
}
