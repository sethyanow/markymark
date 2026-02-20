//! Private helper functions for the realm index.

/// Resolve a URL-style relative path against a base directory without hitting the filesystem.
///
/// Handles `..` and `.` components by normalising the resulting path component-by-component.
pub(super) fn resolve_relative_path(
    base_dir: &std::path::Path,
    relative_url: &str,
) -> std::path::PathBuf {
    // Start from the base directory and apply each URL segment.
    // PathBuf::pop() clamps at the filesystem root and preserves Windows drive prefixes,
    // so excessive `..` never underflow into a relative path.
    let mut result = base_dir.to_path_buf();
    for segment in relative_url.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                result.pop();
            }
            s => result.push(s),
        }
    }
    result
}

/// Detect whether a URI filename matches a Logseq journal page date pattern.
///
/// Matches filenames of the form `YYYY_MM_DD.md` (underscore) or `YYYY-MM-DD.md` (dash).
/// The filename stem must be exactly 10 characters in the form `YYYY{sep}MM{sep}DD`
/// where both separators are the same character.
///
/// Returns `Some((year, month, day))` on a valid date, `None` otherwise.
/// Does NOT require the file to be in a `journals/` directory — Logseq journal
/// directory is user-configurable.
pub(super) fn detect_journal_date(uri: &str) -> Option<(u16, u8, u8)> {
    // Extract filename from URI (after last '/')
    let filename = uri.rsplit('/').next()?;
    // Strip .md or .markdown extension
    let stem = filename
        .strip_suffix(".md")
        .or_else(|| filename.strip_suffix(".markdown"))?;
    // Stem must be exactly 10 bytes: YYYY{sep}MM{sep}DD
    if stem.len() != 10 {
        return None;
    }
    let bytes = stem.as_bytes();
    // Check separator at positions 4 and 7 (must be the same char: '_' or '-')
    let sep = bytes[4];
    if sep != b'_' && sep != b'-' {
        return None;
    }
    if bytes[7] != sep {
        return None; // mixed separators not allowed
    }
    // Parse year (bytes 0..4), month (5..7), day (8..10)
    let y: u16 = stem[0..4].parse().ok()?;
    let m: u8 = stem[5..7].parse().ok()?;
    let d: u8 = stem[8..10].parse().ok()?;
    // Validate ranges
    if !(1900..=2100).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}
