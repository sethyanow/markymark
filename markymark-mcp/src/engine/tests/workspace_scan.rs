//! Tests for the workspace-scanning helper `collect_documents`.
//!
//! HAZARD: Do NOT use `std::env::set_var` inside these tests. `cargo test`
//! runs them in parallel and env is process-global — mutating it causes
//! data races (v6c 2026-04-22). The gitignore-respect test reads the
//! ambient global gitignore; if your global ignores `public.md` or
//! `private.md`, it will fail loudly with a count mismatch. That's correct
//! behaviour (loud, not silent).

use super::*;

#[tokio::test]
async fn collect_documents_includes_json_alongside_markdown() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("notes.md"), "# Hello\n").unwrap();
    fs::write(dir.path().join("config.json"), "{}").unwrap();
    fs::write(dir.path().join("settings.yaml"), "key: val\n").unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let kinds: Vec<_> = docs.iter().map(|(_, k)| *k).collect();

    assert!(kinds.contains(&DocumentKind::Markdown));
    assert!(kinds.contains(&DocumentKind::Json));
    assert!(kinds.contains(&DocumentKind::Yaml));
    // main.rs should NOT be collected
    assert_eq!(docs.len(), 3);
}

#[tokio::test]
async fn collect_documents_markdown_unchanged() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("readme.md"), "# R\n").unwrap();
    fs::write(dir.path().join("guide.markdown"), "# G\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    assert_eq!(docs.len(), 2);
    assert!(docs.iter().all(|(_, k)| *k == DocumentKind::Markdown));
}

// -- Hygiene tests (marky-v6c: ignore-filter) --

/// marky-v6c 2a: hard-ignore baseline — `.git/`, `target/`, `node_modules/`,
/// `bazel-bin/` must be skipped regardless of user ignore files.
#[tokio::test]
async fn collect_documents_hard_ignore_baseline() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    for bad in [".git", "target", "node_modules", "bazel-bin"] {
        fs::create_dir_all(dir.path().join(bad)).unwrap();
        fs::write(dir.path().join(bad).join("skip.md"), "# skip\n").unwrap();
    }
    fs::write(dir.path().join("real.md"), "# real\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let names: Vec<_> = docs
        .iter()
        .filter_map(|(p, _)| p.file_name().and_then(|n| n.to_str().map(String::from)))
        .collect();

    assert_eq!(docs.len(), 1, "expected only real.md, got {:?}", names);
    assert!(docs[0].0.ends_with("real.md"), "got {:?}", docs[0].0);
}

/// marky-v6c 2b: `.gitignore` respect — files listed in .gitignore are excluded.
///
/// Note: this test reads the user's ambient global gitignore (via `ignore`'s
/// `standard_filters` default). If your global gitignore excludes `public.md`
/// or `private.md`, the test will fail loudly with a count mismatch — not
/// silently. We do not mutate env (unsound in parallel tests).
#[tokio::test]
async fn collect_documents_respects_gitignore() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    fs::write(dir.path().join(".gitignore"), "private.md\n").unwrap();
    fs::write(dir.path().join("public.md"), "# public\n").unwrap();
    fs::write(dir.path().join("private.md"), "# private\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let names: Vec<_> = docs
        .iter()
        .filter_map(|(p, _)| p.file_name().and_then(|n| n.to_str().map(String::from)))
        .collect();

    assert_eq!(docs.len(), 1, "expected only public.md, got {:?}", names);
    assert!(docs[0].0.ends_with("public.md"), "got {:?}", docs[0].0);
}

/// marky-v6c 2c: opt-in-safe — a workspace with NO ignore files of any kind
/// still gets the hard-ignore baseline. Distinct from 2a: this one verifies
/// baseline fires without any .gitignore / .ignore / .markymarkignore trigger.
#[tokio::test]
async fn collect_documents_baseline_without_ignore_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // No .gitignore, no .ignore, no .markymarkignore — but baseline must still apply.
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("target").join("artifact.md"), "# skip\n").unwrap();
    fs::write(dir.path().join("real.md"), "# real\n").unwrap();

    let docs = helpers::collect_documents(dir.path());

    assert_eq!(docs.len(), 1, "baseline must apply without any ignore file");
    assert!(docs[0].0.ends_with("real.md"));
}

/// marky-v6c 2d: sort determinism — output is lexicographically sorted.
#[tokio::test]
async fn collect_documents_output_is_sorted() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    // Create out of order; assert returned order is alphabetical.
    fs::write(dir.path().join("c.md"), "# c\n").unwrap();
    fs::write(dir.path().join("a.md"), "# a\n").unwrap();
    fs::write(dir.path().join("b.md"), "# b\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let names: Vec<_> = docs
        .iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    assert_eq!(names, vec!["a.md", "b.md", "c.md"]);
}

/// marky-v6c 2e: symlink-cycle safety — a dir containing a symlink to its
/// parent must NOT cause an infinite walk. The walker follows links (needed
/// for Bazel runfiles) but relies on `ignore::Walk`'s built-in cycle
/// detection to terminate. Wall-clock budget of 5s catches regressions that
/// break cycle detection.
#[cfg(unix)]
#[tokio::test]
async fn collect_documents_terminates_on_symlink_cycle() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("real.md"), "# real\n").unwrap();

    // Build a loop: dir/loop/parent -> ..  (i.e. back to dir/loop)
    let loop_dir = dir.path().join("loop");
    fs::create_dir(&loop_dir).unwrap();
    std::os::unix::fs::symlink("..", loop_dir.join("parent")).expect("failed to create symlink");

    let start = std::time::Instant::now();
    let docs = helpers::collect_documents(dir.path());
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "walk exceeded 5s budget ({:?}) — is follow_links enabled?",
        elapsed
    );
    // Positive check: real.md is in the result (otherwise test passes for wrong reason).
    assert!(
        docs.iter().any(|(p, _)| p.ends_with("real.md")),
        "expected real.md in {:?}",
        docs
    );
}

// -- Adversarial stress tests (marky-v6c) --

/// Adversarial/Empty: walking an empty directory returns an empty Vec.
#[tokio::test]
async fn collect_documents_empty_dir() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let docs = helpers::collect_documents(dir.path());
    assert!(
        docs.is_empty(),
        "empty dir must return empty, got {:?}",
        docs
    );
}

/// Adversarial/Type-boundary: a non-existent root is handled without panic.
/// Caller contract is `validate_workspace_root` enforces existence, but the
/// function itself must not panic if called on a bogus path.
#[tokio::test]
async fn collect_documents_nonexistent_root_does_not_panic() {
    let bogus = std::path::PathBuf::from("/nonexistent/path/that/does/not/exist/12345");
    let docs = helpers::collect_documents(&bogus);
    assert!(
        docs.is_empty(),
        "bogus root must return empty, got {:?}",
        docs
    );
}

/// Adversarial/Type-boundary: a root that is a regular file (not a dir) must
/// not panic. `ignore::WalkBuilder::new(file)` yields the file itself as an
/// entry — our filter takes care of accepting or rejecting it.
#[tokio::test]
async fn collect_documents_root_is_a_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let file = dir.path().join("alone.md");
    fs::write(&file, "# alone\n").unwrap();

    // Should not panic when invoked on the file itself.
    let docs = helpers::collect_documents(&file);
    // Accept either interpretation: walker yields the file (1) or rejects it (0).
    // Behavior is not strongly specified at this boundary; non-panic is what matters.
    assert!(
        docs.len() <= 1,
        "walk on a single-file root must yield at most 1 entry, got {:?}",
        docs
    );
}

/// Adversarial/Encoding-boundary: Unicode filenames (multi-byte UTF-8) are
/// collected correctly. `DocumentKind::from_path` uses `OsStr`-based extension
/// lookup — no UTF-8 assumption in the extension check, but filenames can be
/// long multi-byte strings.
#[tokio::test]
async fn collect_documents_unicode_filenames() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    // Mixed scripts, emoji, combining marks.
    fs::write(dir.path().join("日本語.md"), "# jp\n").unwrap();
    fs::write(dir.path().join("🦀.md"), "# crab\n").unwrap();
    fs::write(dir.path().join("café.md"), "# fr\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    assert_eq!(docs.len(), 3, "all unicode .md files must be collected");
    assert!(docs.iter().all(|(_, k)| *k == DocumentKind::Markdown));
}

/// Adversarial/Second-run: function is deterministic across repeat invocations
/// on the same filesystem state. Sort order and content must be identical.
#[tokio::test]
async fn collect_documents_is_deterministic_across_runs() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    for name in ["a.md", "b.md", "c.json", "d.yaml"] {
        fs::write(dir.path().join(name), "x").unwrap();
    }

    let first = helpers::collect_documents(dir.path());
    let second = helpers::collect_documents(dir.path());
    let third = helpers::collect_documents(dir.path());

    assert_eq!(first, second, "run 1 vs run 2 must match");
    assert_eq!(second, third, "run 2 vs run 3 must match");
    assert_eq!(first.len(), 4);
}

/// Adversarial/Dense: deeply nested directory does not exhaust stack.
/// `ignore::Walk` is iterative (not recursive) — this verifies it.
#[tokio::test]
async fn collect_documents_deep_nesting() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mut current = dir.path().to_path_buf();
    for i in 0..64 {
        current = current.join(format!("level{i}"));
        fs::create_dir(&current).unwrap();
    }
    fs::write(current.join("deep.md"), "# deep\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    assert_eq!(docs.len(), 1, "deep.md must be found in 64-level nesting");
    assert!(docs[0].0.ends_with("deep.md"));
}

/// Adversarial/Self-referential (broken): a dangling symlink (target missing)
/// must not cause the walker to panic or abort the whole walk.
#[cfg(unix)]
#[tokio::test]
async fn collect_documents_broken_symlink_is_skipped() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("real.md"), "# real\n").unwrap();

    // Symlink pointing at a file that does not exist.
    std::os::unix::fs::symlink(
        dir.path().join("missing_target.md"),
        dir.path().join("dangling.md"),
    )
    .unwrap();

    let docs = helpers::collect_documents(dir.path());
    // real.md must still be found; dangling.md may or may not appear — we do
    // not specify. Non-panic + finding real.md is the contract.
    assert!(
        docs.iter().any(|(p, _)| p.ends_with("real.md")),
        "real.md missing from {:?}",
        docs
    );
}

/// Adversarial/Semantically-hostile: a regular file named `target` at the root
/// must NOT be rejected (hard-ignore is directory-only). Guards against the
/// SRE finding about `filter_entry` running on files.
#[tokio::test]
async fn collect_documents_file_named_target_is_not_filtered() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    // Plain file (not dir) with a name matching HARD_IGNORE_DIRS. The walker
    // must NOT treat this as a hard-ignore — only directories are rejected.
    fs::write(dir.path().join("target"), "not a dir").unwrap();
    // DocumentKind::from_path checks the extension, so a file named `target`
    // (no extension) is not collected anyway. Confirm via a matching document
    // with the same stem:
    fs::write(dir.path().join("target.md"), "# legit content\n").unwrap();

    let docs = helpers::collect_documents(dir.path());
    let names: Vec<_> = docs
        .iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    assert!(
        names.iter().any(|n| n == "target.md"),
        "regular file `target.md` must not be rejected by dir hard-ignore, got {:?}",
        names
    );
}

/// Ignored timing benchmark. Run manually with:
///   cargo test -p markymark-mcp --lib v6c_speedup_probe -- --ignored --nocapture
/// Prints wall-clock of `collect_documents` on the real worktree root. The
/// prior session observed > 60s hang without the ignore filter; the new impl
/// must complete quickly.
#[tokio::test]
#[ignore]
async fn v6c_speedup_probe() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let start = std::time::Instant::now();
    let docs = helpers::collect_documents(&root);
    let elapsed = start.elapsed();
    println!(
        "v6c_speedup_probe: {} docs collected from {} in {:?}",
        docs.len(),
        root.display(),
        elapsed
    );
}
