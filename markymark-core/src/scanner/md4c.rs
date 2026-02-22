//! Scan backend implementations using Zig SIMD kernels and md4c FFI.

use super::types::*;

// ---------------------------------------------------------------------------
// ZigScanBackend implementation (behind zig-kernels feature)
// ---------------------------------------------------------------------------

/// Zig SIMD-accelerated scan backend.
///
/// This backend delegates to the `markymark-kernels` FFI functions for
/// SIMD-accelerated markdown element extraction.
#[cfg(feature = "zig-kernels")]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZigScanBackend;

#[cfg(feature = "zig-kernels")]
impl super::ScanBackend for ZigScanBackend {
    fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
        markymark_kernels::scan::scan_headings(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|h| HeadingResult {
                        text: h.text,
                        offset: h.offset,
                        level: h.level,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
        markymark_kernels::scan::scan_links(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|l| LinkResult {
                        offset: l.offset,
                        text: l.text,
                        target: l.target,
                        link_type: match l.link_type {
                            markymark_kernels::scan::LinkType::Markdown => ScanLinkType::Markdown,
                            markymark_kernels::scan::LinkType::Wiki => ScanLinkType::Wiki,
                        },
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError> {
        markymark_kernels::scan::scan_tags(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|t| TagResult {
                        name: t.name,
                        offset: t.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
        markymark_kernels::scan::scan_block_ids(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|b| BlockIdResult {
                        id: b.id,
                        offset: b.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError> {
        Ok(markymark_kernels::tokens::estimate_tokens(text))
    }
}

// ---------------------------------------------------------------------------
// Md4cScanBackend implementation (behind zig-kernels feature)
// ---------------------------------------------------------------------------

/// md4c-based scan backend using single-pass streaming extraction.
///
/// Uses the Zig md4c ExtractionRenderer via FFI for heading and link
/// extraction. Delegates tags, block IDs, and token estimation to the
/// same Zig SIMD kernels used by [`ZigScanBackend`].
#[cfg(feature = "zig-kernels")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Md4cScanBackend;

#[cfg(feature = "zig-kernels")]
impl super::ScanBackend for Md4cScanBackend {
    fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .headings
                    .into_iter()
                    .map(map_md4c_heading)
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| extraction.links.into_iter().map(map_md4c_link).collect())
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_tags(&self, text: &str) -> Result<Vec<TagResult>, ScanError> {
        markymark_kernels::scan::scan_tags(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|t| TagResult {
                        name: t.name,
                        offset: t.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_block_ids(&self, text: &str) -> Result<Vec<BlockIdResult>, ScanError> {
        markymark_kernels::scan::scan_block_ids(text)
            .map(|results| {
                results
                    .into_iter()
                    .map(|b| BlockIdResult {
                        id: b.id,
                        offset: b.offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn estimate_tokens(&self, text: &str) -> Result<u32, ScanError> {
        Ok(markymark_kernels::tokens::estimate_tokens(text))
    }

    fn scan_code_spans(&self, text: &str) -> Result<Vec<CodeSpanResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .code_spans
                    .into_iter()
                    .map(|cs| CodeSpanResult {
                        text: cs.text,
                        offset: cs.source_offset,
                        end_offset: cs.end_offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_tasks(&self, text: &str) -> Result<Vec<TaskResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .tasks
                    .into_iter()
                    .map(|t| TaskResult {
                        state: t.state,
                        text: t.text,
                        offset: t.source_offset,
                        end_offset: t.end_offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_embeds(&self, text: &str) -> Result<Vec<EmbedResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .embeds
                    .into_iter()
                    .map(|e| EmbedResult {
                        target: e.target,
                        offset: e.source_offset,
                        end_offset: e.end_offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_callouts(&self, text: &str) -> Result<Vec<CalloutResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .callouts
                    .into_iter()
                    .map(|c| CalloutResult {
                        callout_type: c.callout_type,
                        title: c.title,
                        offset: c.source_offset,
                        end_offset: c.end_offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_block_refs(&self, text: &str) -> Result<Vec<BlockRefResult>, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| {
                extraction
                    .block_refs
                    .into_iter()
                    .map(|br| BlockRefResult {
                        uuid: br.uuid,
                        offset: br.source_offset,
                    })
                    .collect()
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }

    fn scan_all(&self, text: &str) -> Result<ScanAllResult, ScanError> {
        markymark_kernels::md4c::extract_md4c(text)
            .map(|extraction| ScanAllResult {
                headings: extraction
                    .headings
                    .into_iter()
                    .map(map_md4c_heading)
                    .collect(),
                links: extraction.links.into_iter().map(map_md4c_link).collect(),
                code_spans: extraction
                    .code_spans
                    .into_iter()
                    .map(|cs| CodeSpanResult {
                        text: cs.text,
                        offset: cs.source_offset,
                        end_offset: cs.end_offset,
                    })
                    .collect(),
                tasks: extraction
                    .tasks
                    .into_iter()
                    .map(|t| TaskResult {
                        state: t.state,
                        text: t.text,
                        offset: t.source_offset,
                        end_offset: t.end_offset,
                    })
                    .collect(),
                embeds: extraction
                    .embeds
                    .into_iter()
                    .map(|e| EmbedResult {
                        target: e.target,
                        offset: e.source_offset,
                        end_offset: e.end_offset,
                    })
                    .collect(),
                callouts: extraction
                    .callouts
                    .into_iter()
                    .map(|c| CalloutResult {
                        callout_type: c.callout_type,
                        title: c.title,
                        offset: c.source_offset,
                        end_offset: c.end_offset,
                    })
                    .collect(),
                block_refs: extraction
                    .block_refs
                    .into_iter()
                    .map(|br| BlockRefResult {
                        uuid: br.uuid,
                        offset: br.source_offset,
                    })
                    .collect(),
            })
            .map_err(|e| ScanError::InternalError(e.to_string()))
    }
}

#[cfg(feature = "zig-kernels")]
#[inline]
fn map_md4c_heading(h: markymark_kernels::md4c::Md4cHeading) -> HeadingResult {
    HeadingResult {
        text: h.text,
        offset: h.source_offset,
        level: h.level,
    }
}

#[cfg(feature = "zig-kernels")]
#[inline]
fn map_md4c_link(l: markymark_kernels::md4c::Md4cLink) -> LinkResult {
    LinkResult {
        offset: l.source_offset,
        text: l.text,
        target: l.target,
        link_type: if l.is_wiki {
            ScanLinkType::Wiki
        } else {
            ScanLinkType::Markdown
        },
    }
}
