#![cfg(feature = "zig-kernels")]

use std::fs;
use std::path::{Path, PathBuf};

use markymark_core::scanner::ZigScanBackend;
use markymark_index::DocumentIndex;
use markymark_parser::Parser;

#[derive(Debug, Default, Clone, Copy)]
struct ElementCounts {
    headings: usize,
    wiki_links: usize,
    markdown_links: usize,
    tags: usize,
    block_ids: usize,
}

impl ElementCounts {
    fn total(self) -> usize {
        self.headings + self.wiki_links + self.markdown_links + self.tags + self.block_ids
    }

    fn add_assign(&mut self, other: Self) {
        self.headings += other.headings;
        self.wiki_links += other.wiki_links;
        self.markdown_links += other.markdown_links;
        self.tags += other.tags;
        self.block_ids += other.block_ids;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ParityDelta {
    false_positives: ElementCounts,
    false_negatives: ElementCounts,
}

impl ParityDelta {
    fn total_false_positives(self) -> usize {
        self.false_positives.total()
    }

    fn total_false_negatives(self) -> usize {
        self.false_negatives.total()
    }
}

#[derive(Debug)]
struct FixtureCase {
    name: &'static str,
    markdown: &'static str,
    known_setext_gap: bool,
    known_frontmatter_gap: bool,
    code_heavy: bool,
}

fn fixture_cases() -> Vec<FixtureCase> {
    vec![
        FixtureCase {
            name: "simple_atx_headings",
            markdown: "# Root\n\n## Child\n\n### Leaf\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "wiki_and_markdown_links",
            markdown: "# Links\n\nSee [[Page Name]] and [Rust](https://www.rust-lang.org).\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "alias_wiki_links",
            markdown: "# Alias\n\nUse [[target-page|Display Name]] for short refs.\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "tags_and_block_ids",
            markdown: "# Tags\n\nUse #project and #area/docs\n\nParagraph ^block-a\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "markdown_anchor_links",
            markdown: "# Anchor\n\nSee [docs](./guide.md).\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "empty_document",
            markdown: "",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "no_structural_elements",
            markdown: "This paragraph has no links, tags, headings, or block IDs.",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "code_fence_false_positive_candidate",
            markdown: "# Code\n\n```rust\nlet url = \"https://example.com\";\nlet tag = \"#inside_code\";\nlet block = \"^inside\";\n```\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: true,
        },
        FixtureCase {
            name: "setext_heading_known_gap",
            markdown: "Heading One\n===========\n\nParagraph with [[Wiki]] link.\n",
            known_setext_gap: true,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
        FixtureCase {
            name: "frontmatter_known_gap",
            markdown: "---\ntitle: Sample\ntags:\n  - from-frontmatter\n---\n\n# Body\n\nText #bodytag\n",
            known_setext_gap: false,
            known_frontmatter_gap: true,
            code_heavy: false,
        },
        FixtureCase {
            name: "mixed_realistic",
            markdown: "# Project Notes\n\nUse [[Roadmap]] and [Spec](./spec.md).\n\n- item ^item-1\n- item #listtag\n",
            known_setext_gap: false,
            known_frontmatter_gap: false,
            code_heavy: false,
        },
    ]
}

fn index_from_ast(source: &str) -> Option<DocumentIndex> {
    let mut parser = Parser::new().expect("parser init");
    match parser.parse(source) {
        Ok(ast) => Some(DocumentIndex::from_ast(ast)),
        Err(err) => {
            eprintln!("tree-sitter parse failed: {err}");
            None
        }
    }
}

fn index_from_scan(source: &str) -> DocumentIndex {
    let backend = ZigScanBackend;
    DocumentIndex::from_scan(source, &backend)
}

fn count_elements(index: &DocumentIndex) -> ElementCounts {
    ElementCounts {
        headings: index.headings().len(),
        wiki_links: index.wiki_links().len(),
        markdown_links: index.markdown_links().len(),
        tags: index.tags().len(),
        block_ids: index.block_ids().count(),
    }
}

fn compare_counts(ast: ElementCounts, scan: ElementCounts) -> ParityDelta {
    ParityDelta {
        false_positives: ElementCounts {
            headings: scan.headings.saturating_sub(ast.headings),
            wiki_links: scan.wiki_links.saturating_sub(ast.wiki_links),
            markdown_links: scan.markdown_links.saturating_sub(ast.markdown_links),
            tags: scan.tags.saturating_sub(ast.tags),
            block_ids: scan.block_ids.saturating_sub(ast.block_ids),
        },
        false_negatives: ElementCounts {
            headings: ast.headings.saturating_sub(scan.headings),
            wiki_links: ast.wiki_links.saturating_sub(scan.wiki_links),
            markdown_links: ast.markdown_links.saturating_sub(scan.markdown_links),
            tags: ast.tags.saturating_sub(scan.tags),
            block_ids: ast.block_ids.saturating_sub(scan.block_ids),
        },
    }
}

fn has_frontmatter(text: &str) -> bool {
    text.starts_with("---\n") || text.starts_with("+++\n")
}

fn contains_fenced_code_block(text: &str) -> bool {
    text.contains("```") || text.contains("~~~")
}

fn contains_setext_heading(text: &str) -> bool {
    let mut prev = "";
    for line in text.lines() {
        let trimmed = line.trim();
        // Skip YAML frontmatter delimiters — "---" at document boundaries is not a setext underline.
        if trimmed == "---" {
            prev = line;
            continue;
        }
        if !prev.trim().is_empty()
            && trimmed.len() >= 3
            && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
        {
            return true;
        }
        prev = line;
    }
    false
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return,
    };
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        if path.is_dir() {
            collect_markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[test]
fn test_extraction_parity_handcrafted_fixtures() {
    let fixtures = fixture_cases();
    assert!(
        fixtures.len() >= 10,
        "expected at least 10 handcrafted fixtures"
    );

    for fixture in fixtures {
        let ast = index_from_ast(fixture.markdown).expect("fixture should parse");
        let scan = index_from_scan(fixture.markdown);
        let ast_counts = count_elements(&ast);
        let scan_counts = count_elements(&scan);

        if fixture.known_setext_gap {
            assert!(
                scan_counts.headings <= ast_counts.headings,
                "{name}: scan should not exceed AST heading count for known setext gap",
                name = fixture.name
            );
            continue;
        }

        if fixture.known_frontmatter_gap {
            assert!(
                scan_counts.tags >= ast_counts.tags,
                "{name}: scan may over-count tags from frontmatter",
                name = fixture.name
            );
            continue;
        }

        if fixture.code_heavy {
            assert_eq!(
                ast_counts.headings, scan_counts.headings,
                "{}: code-heavy fixture should still preserve heading count",
                fixture.name
            );
            continue;
        }

        assert_eq!(
            ast_counts.headings, scan_counts.headings,
            "{}: headings should match",
            fixture.name
        );
        assert_eq!(
            ast_counts.wiki_links, scan_counts.wiki_links,
            "{}: wiki links should match",
            fixture.name
        );
        assert_eq!(
            ast_counts.markdown_links, scan_counts.markdown_links,
            "{}: markdown links should match",
            fixture.name
        );
        assert_eq!(
            ast_counts.tags, scan_counts.tags,
            "{}: tags should match",
            fixture.name
        );
        assert_eq!(
            ast_counts.block_ids, scan_counts.block_ids,
            "{}: block IDs should match",
            fixture.name
        );
    }
}

#[test]
fn test_extraction_parity_docs_corpus_and_report() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let docs_root = workspace_root.join("docs");
    let report_path = docs_root.join("benchmarks/extraction-parity.md");

    let mut files = Vec::new();
    collect_markdown_files(&docs_root, &mut files);
    files.sort();
    files.retain(|path| path != &report_path);

    assert!(
        files.len() >= 50,
        "expected docs corpus to include ~50 markdown files, got {}",
        files.len()
    );

    let mut raw_tree = ElementCounts::default();
    let mut raw_scan = ElementCounts::default();
    let mut raw_fp = ElementCounts::default();
    let mut raw_fn = ElementCounts::default();

    let mut adjusted_tree = ElementCounts::default();
    let mut adjusted_fp = ElementCounts::default();
    let mut adjusted_fn = ElementCounts::default();

    let mut parsed_files = 0usize;
    let mut skipped_files = 0usize;
    let mut setext_docs = 0usize;
    let mut frontmatter_docs = 0usize;
    let mut code_block_docs = 0usize;
    let mut code_block_fp_docs = 0usize;
    let mut code_block_fp_events = 0usize;

    let mut mismatches = Vec::new();

    for path in files {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                skipped_files += 1;
                eprintln!("skipping unreadable file {}: {err}", path.display());
                continue;
            }
        };

        let Some(ast) = index_from_ast(&text) else {
            skipped_files += 1;
            eprintln!("skipping parse-failed file {}", path.display());
            continue;
        };
        let scan = index_from_scan(&text);

        let ast_counts = count_elements(&ast);
        let scan_counts = count_elements(&scan);
        let delta = compare_counts(ast_counts, scan_counts);

        parsed_files += 1;
        raw_tree.add_assign(ast_counts);
        raw_scan.add_assign(scan_counts);
        raw_fp.add_assign(delta.false_positives);
        raw_fn.add_assign(delta.false_negatives);

        let has_setext = contains_setext_heading(&text);
        let has_frontmatter = has_frontmatter(&text);
        let has_code_blocks = contains_fenced_code_block(&text);

        if has_setext {
            setext_docs += 1;
        }
        if has_frontmatter {
            frontmatter_docs += 1;
        }
        if has_code_blocks {
            code_block_docs += 1;
            if delta.total_false_positives() > 0 {
                code_block_fp_docs += 1;
                code_block_fp_events += delta.total_false_positives();
            }
        }

        if !has_setext && !has_frontmatter {
            adjusted_tree.add_assign(ast_counts);
            adjusted_fp.add_assign(delta.false_positives);
            adjusted_fn.add_assign(delta.false_negatives);
        }

        if delta.total_false_positives() > 0 || delta.total_false_negatives() > 0 {
            let rel = path
                .strip_prefix(&workspace_root)
                .unwrap_or(path.as_path())
                .display()
                .to_string();
            mismatches.push((
                rel,
                delta.total_false_positives(),
                delta.total_false_negatives(),
                has_setext,
                has_frontmatter,
            ));
        }
    }

    mismatches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

    let raw_fp_rate = percent(raw_fp.total(), raw_tree.total());
    let raw_fn_rate = percent(raw_fn.total(), raw_tree.total());
    let adjusted_fp_rate = percent(adjusted_fp.total(), adjusted_tree.total());
    let adjusted_fn_rate = percent(adjusted_fn.total(), adjusted_tree.total());
    let code_block_fp_doc_rate = percent(code_block_fp_docs, code_block_docs);

    let mut report = String::new();
    report.push_str("# Extraction Parity: Zig SIMD vs tree-sitter\n\n");
    report.push_str("This report compares Tier 1 (Zig scan via `DocumentIndex::from_scan`) against Tier 2 (tree-sitter via `DocumentIndex::from_ast`) across the local `docs/` corpus.\n\n");
    report.push_str("## Corpus\n\n");
    report.push_str(&format!(
        "- Parsed markdown files: {}\n- Skipped files (read/parse failures): {}\n- Files with setext headings: {}\n- Files with frontmatter: {}\n- Files with fenced code blocks: {}\n\n",
        parsed_files, skipped_files, setext_docs, frontmatter_docs, code_block_docs
    ));
    report.push_str("## Aggregate Counts\n\n");
    report.push_str("| Metric | AST Total | Scan Total | False Positives | False Negatives |\n");
    report.push_str("|---|---:|---:|---:|---:|\n");
    report.push_str(&format!(
        "| Headings | {} | {} | {} | {} |\n",
        raw_tree.headings, raw_scan.headings, raw_fp.headings, raw_fn.headings
    ));
    report.push_str(&format!(
        "| Wiki Links | {} | {} | {} | {} |\n",
        raw_tree.wiki_links, raw_scan.wiki_links, raw_fp.wiki_links, raw_fn.wiki_links
    ));
    report.push_str(&format!(
        "| Markdown Links | {} | {} | {} | {} |\n",
        raw_tree.markdown_links,
        raw_scan.markdown_links,
        raw_fp.markdown_links,
        raw_fn.markdown_links
    ));
    report.push_str(&format!(
        "| Tags | {} | {} | {} | {} |\n",
        raw_tree.tags, raw_scan.tags, raw_fp.tags, raw_fn.tags
    ));
    report.push_str(&format!(
        "| Block IDs | {} | {} | {} | {} |\n\n",
        raw_tree.block_ids, raw_scan.block_ids, raw_fp.block_ids, raw_fn.block_ids
    ));

    report.push_str("## Rates\n\n");
    report.push_str(&format!(
        "- Raw false positive rate: {:.2}%\n- Raw false negative rate: {:.2}%\n- Adjusted false positive rate (excluding known setext/frontmatter gaps): {:.2}%\n- Adjusted false negative rate (excluding known setext/frontmatter gaps): {:.2}%\n- Code block false-positive doc rate: {:.2}% ({} / {})\n- Code block false-positive events: {}\n\n",
        raw_fp_rate * 100.0,
        raw_fn_rate * 100.0,
        adjusted_fp_rate * 100.0,
        adjusted_fn_rate * 100.0,
        code_block_fp_doc_rate * 100.0,
        code_block_fp_docs,
        code_block_docs,
        code_block_fp_events
    ));

    report.push_str("## Known Differences\n\n");
    report.push_str("- Setext headings are a known gap for scan extraction (ATX-focused).\n");
    report.push_str("- Frontmatter can produce extra scan-side tags/links because scan is lexical and not AST-contextual.\n");
    report.push_str(
        "- Fenced code blocks are the primary expected source of scan false positives.\n\n",
    );

    report.push_str("## Top Mismatch Files (by false positives)\n\n");
    report.push_str("| File | False Positives | False Negatives | Setext | Frontmatter |\n");
    report.push_str("|---|---:|---:|:---:|:---:|\n");
    for (file, fp, fn_count, has_setext, has_frontmatter) in mismatches.iter().take(20) {
        report.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            file,
            fp,
            fn_count,
            if *has_setext { "yes" } else { "no" },
            if *has_frontmatter { "yes" } else { "no" },
        ));
    }
    report.push('\n');

    // Only write the report to the repo when explicitly requested via env var,
    // to avoid mutating the working tree in CI or normal test runs.
    if std::env::var("WRITE_PARITY_REPORT").is_ok() {
        fs::create_dir_all(report_path.parent().expect("report parent dir"))
            .expect("create report dir");
        fs::write(&report_path, report).expect("write parity report");
    }

    assert!(
        adjusted_fp_rate < 0.05,
        "adjusted false positive rate {:.2}% exceeds 5% threshold (see {})",
        adjusted_fp_rate * 100.0,
        report_path.display()
    );
}
