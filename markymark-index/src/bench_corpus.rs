//! Synthetic benchmark corpus generation.

/// Target synthetic document sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocSizeTier {
    /// ~1 KiB document.
    Small,
    /// ~10 KiB document.
    Medium,
    /// ~100 KiB document.
    Large,
}

impl DocSizeTier {
    fn target_bytes(self) -> usize {
        match self {
            Self::Small => 1_024,
            Self::Medium => 10_240,
            Self::Large => 102_400,
        }
    }
}

/// Build one synthetic markdown document at the requested size tier.
#[must_use]
pub fn build_sized_doc(index: usize, tier: DocSizeTier) -> String {
    let target = tier.target_bytes();
    let mut out = String::with_capacity(target + 256);

    out.push_str(&format!(
        "# Synthetic Document {index}\n\n\
         title: synthetic-{index}\n\
         kind: benchmark\n\
         ---\n\n\
         ## Section A\n\
         This document exercises [[wiki-links]] #tags and [markdown links](https://example.com/{index}).\n\n\
         <task id=\"t{index}\" owner=\"bench\">alpha</task>\n\n"
    ));

    // Repeat structured markdown to mimic mixed PKM/dev notes while controlling size.
    let chunk = format!(
        "- item {index}\n\
         - nested detail for ^block-{index}\n\
         - xml <meta key=\"k{index}\" value=\"v{index}\" />\n\
         - json-ish {{\"id\": {index}, \"state\": \"active\"}}\n\
         \n"
    );

    while out.len() < target {
        out.push_str(&chunk);
    }

    out
}

/// Build a mixed-size synthetic corpus for scale benchmarks.
///
/// Size distribution cycles `Small -> Medium -> Large`.
#[must_use]
pub fn build_mixed_size_corpus(doc_count: usize) -> Vec<String> {
    let mut docs = Vec::with_capacity(doc_count);
    for i in 0..doc_count {
        let tier = match i % 3 {
            0 => DocSizeTier::Small,
            1 => DocSizeTier::Medium,
            _ => DocSizeTier::Large,
        };
        docs.push(build_sized_doc(i, tier));
    }
    docs
}

#[cfg(test)]
mod tests {
    use super::{build_mixed_size_corpus, build_sized_doc, DocSizeTier};

    #[test]
    fn sized_docs_meet_minimum_target_size() {
        let small = build_sized_doc(1, DocSizeTier::Small);
        let medium = build_sized_doc(2, DocSizeTier::Medium);
        let large = build_sized_doc(3, DocSizeTier::Large);

        assert!(small.len() >= 1_024);
        assert!(medium.len() >= 10_240);
        assert!(large.len() >= 102_400);
    }

    #[test]
    fn mixed_corpus_respects_requested_count() {
        let docs = build_mixed_size_corpus(37);
        assert_eq!(docs.len(), 37);
    }

    #[test]
    fn mixed_corpus_contains_multiple_size_bands() {
        let docs = build_mixed_size_corpus(6);
        let lengths: Vec<usize> = docs.iter().map(String::len).collect();

        assert!(lengths[0] < lengths[1]);
        assert!(lengths[1] < lengths[2]);
        assert!(lengths[3] < lengths[4]);
        assert!(lengths[4] < lengths[5]);
    }
}
