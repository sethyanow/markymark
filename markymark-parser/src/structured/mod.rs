//! Parsers for structured (non-markdown) document formats.
//!
//! Each format has its own submodule that produces a [`StructuredAst`] with
//! byte-accurate source ranges. The [`parse_structured`] function dispatches
//! to the appropriate parser based on [`DocumentKind`].

mod json;
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
        DocumentKind::Yaml => yaml::parse_yaml(source),
        DocumentKind::Toml => toml::parse_toml(source),
        DocumentKind::JsonC
        | DocumentKind::Json5
        | DocumentKind::JsonLines
        | DocumentKind::DotEnv
        | DocumentKind::Ini => Err(CoreError::NotImplemented(format!(
            "{kind} parser not yet implemented"
        ))),
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
    fn test_parse_structured_dispatch_unimplemented() {
        let result = parse_structured("[section]\nkey = value", DocumentKind::Ini);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not yet implemented"));
    }
}
