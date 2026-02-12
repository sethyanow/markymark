//! Dual-process LSP alignment tests: marksman vs markymark.
//!
//! Spawns both `marksman server` and `markymark --lsp` as child processes,
//! sends identical LSP requests over stdio, normalizes responses, and
//! classifies each comparison as Match/Superset/Mismatch/MarksmanOnly/MarkymarkOnly.
//!
//! Tests skip gracefully when marksman is not found (CI compatibility).

mod alignment_support;

use alignment_support::{
    compare_responses, corpus_dir, markymark_bin, path_to_uri, run_with_timeout, truncate_json,
    AlignmentReport, AlignmentResult, LspProcess, MethodComparison,
};
use serde_json::Value;
use std::path::Path;

// ---------------------------------------------------------------------------
// DualLspHarness — wraps both servers
// ---------------------------------------------------------------------------

struct DualLspHarness {
    marksman: LspProcess,
    markymark: LspProcess,
}

impl DualLspHarness {
    /// Spawn both servers, open all corpus files, drain notifications.
    fn setup_workspace(marksman_path: &Path) -> Self {
        let markymark_path = markymark_bin();
        let corpus = corpus_dir();

        let mut marksman = LspProcess::spawn(marksman_path, &["server"], &corpus, "marksman");
        let mut markymark = LspProcess::spawn(&markymark_path, &["--lsp"], &corpus, "markymark");

        let corpus_files = [
            "basic.md",
            "links.md",
            "cross-refs.md",
            "edge-cases.md",
            "xml-tags.md",
        ];
        for filename in &corpus_files {
            let path = corpus.join(filename);
            if path.exists() {
                marksman.open_file(&path);
                markymark.open_file(&path);
            }
        }

        marksman.drain_notifications();
        markymark.drain_notifications();

        Self {
            marksman,
            markymark,
        }
    }

    /// Send a request to both servers and compare responses.
    fn compare(&mut self, method: &str, params: Value, file: &str) -> MethodComparison {
        let ms_result = self.marksman.send_request(method, params.clone());
        let mm_result = self.markymark.send_request(method, params);

        let result = compare_responses(method, &ms_result, &mm_result);
        let notes = match &result {
            AlignmentResult::Mismatch {
                marksman,
                markymark,
            } => {
                format!(
                    "marksman={}, markymark={}",
                    truncate_json(marksman, 100),
                    truncate_json(markymark, 100),
                )
            }
            _ => String::new(),
        };

        MethodComparison {
            method: method.to_string(),
            file: file.to_string(),
            result,
            notes,
        }
    }

    fn shutdown(self) -> (i32, i32) {
        let ms_code = self.marksman.shutdown_and_exit();
        let mm_code = self.markymark.shutdown_and_exit();
        (ms_code, mm_code)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_alignment_both_servers_initialize() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let harness = DualLspHarness::setup_workspace(&marksman_path);
        let (ms_code, mm_code) = harness.shutdown();
        assert_eq!(ms_code, 0, "marksman should exit cleanly");
        assert_eq!(mm_code, 0, "markymark should exit cleanly");
    });
}

#[test]
fn test_align_definition_wiki_link() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("links.md"));

        let comp = harness.compare(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": 25 }
            }),
            "links.md",
        );

        eprintln!("[definition] {}", comp.result);
        assert!(
            !matches!(comp.result, AlignmentResult::MarksmanOnly),
            "markymark should handle definition requests"
        );
        harness.shutdown();
    });
}

#[test]
fn test_align_references_heading() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("basic.md"));

        let comp = harness.compare(
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 2 },
                "context": { "includeDeclaration": true }
            }),
            "basic.md",
        );

        eprintln!("[references] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_align_hover_heading() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("basic.md"));

        let comp = harness.compare(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": 5 }
            }),
            "basic.md",
        );

        eprintln!("[hover] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_align_completion_wiki_link() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("links.md"));

        let comp = harness.compare(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": 24 }
            }),
            "links.md",
        );

        eprintln!("[completion] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_align_document_symbol() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("basic.md"));

        let comp = harness.compare(
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": uri }
            }),
            "basic.md",
        );

        eprintln!("[documentSymbol] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_align_workspace_symbol() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);

        let comp = harness.compare(
            "workspace/symbol",
            serde_json::json!({ "query": "Section" }),
            "(workspace)",
        );

        eprintln!("[workspaceSymbol] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_align_diagnostics() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("links.md"));

        let ms_diags = harness
            .marksman
            .diagnostics
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        let mm_diags = harness
            .markymark
            .diagnostics
            .get(&uri)
            .cloned()
            .unwrap_or_default();

        let ms_val = Value::Array(ms_diags);
        let mm_val = Value::Array(mm_diags);
        let result = compare_responses("diagnostics", &ms_val, &mm_val);
        eprintln!("[diagnostics] {result}");

        harness.shutdown();
    });
}

#[test]
fn test_align_rename_heading() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let uri = path_to_uri(&corpus.join("basic.md"));

        let comp = harness.compare(
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": 5 },
                "newName": "Renamed Section"
            }),
            "basic.md",
        );

        eprintln!("[rename] {}", comp.result);
        harness.shutdown();
    });
}

#[test]
fn test_alignment_full_report() {
    run_with_timeout(|| {
        let marksman_path = require_marksman!();
        let mut harness = DualLspHarness::setup_workspace(&marksman_path);
        let corpus = corpus_dir();
        let mut report = AlignmentReport::new();

        let uri_links = path_to_uri(&corpus.join("links.md"));
        let uri_basic = path_to_uri(&corpus.join("basic.md"));

        // 1. Definition
        let comp = harness.compare(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": &uri_links },
                "position": { "line": 4, "character": 25 }
            }),
            "links.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 2. References
        let comp = harness.compare(
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": &uri_basic },
                "position": { "line": 0, "character": 2 },
                "context": { "includeDeclaration": true }
            }),
            "basic.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 3. Hover
        let comp = harness.compare(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": &uri_basic },
                "position": { "line": 4, "character": 5 }
            }),
            "basic.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 4. Completion
        let comp = harness.compare(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": &uri_links },
                "position": { "line": 4, "character": 24 }
            }),
            "links.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 5. DocumentSymbol
        let comp = harness.compare(
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": &uri_basic }
            }),
            "basic.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 6. WorkspaceSymbol
        let comp = harness.compare(
            "workspace/symbol",
            serde_json::json!({ "query": "Section" }),
            "(workspace)",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // 7. Diagnostics (from collected notifications)
        let ms_diags = harness
            .marksman
            .diagnostics
            .get(&uri_links)
            .cloned()
            .unwrap_or_default();
        let mm_diags = harness
            .markymark
            .diagnostics
            .get(&uri_links)
            .cloned()
            .unwrap_or_default();
        let diag_result = compare_responses(
            "diagnostics",
            &Value::Array(ms_diags),
            &Value::Array(mm_diags),
        );
        report.add("diagnostics", "links.md", diag_result, "");

        // 8. Rename (last — destructive)
        let comp = harness.compare(
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": &uri_basic },
                "position": { "line": 4, "character": 5 },
                "newName": "Renamed Section"
            }),
            "basic.md",
        );
        report.add(&comp.method, &comp.file, comp.result, &comp.notes);

        // Validate report
        assert_eq!(
            report.comparisons.len(),
            8,
            "report should have 8 method comparisons"
        );

        let json = report.to_json();
        assert!(
            json.get("comparisons").is_some(),
            "JSON report should have comparisons"
        );
        assert!(
            json.get("summary").is_some(),
            "JSON report should have summary"
        );

        let summary = report.summary_text();
        assert!(
            summary.contains("Total: 8"),
            "summary should mention 8 comparisons"
        );

        eprintln!("\n{summary}");
        eprintln!("JSON:\n{}", serde_json::to_string_pretty(&json).unwrap());

        harness.shutdown();
    });
}
