//! Sidecar storage types for LLM-enriched document metadata.
//!
//! Sidecar files live in a configurable directory (default `.markymark/`)
//! alongside workspace roots. Each document gets a JSON sidecar containing
//! per-node summaries and a content hash for invalidation.

use serde::{Deserialize, Serialize};

/// Current sidecar format version. Bump when the schema changes.
pub const SIDECAR_VERSION: u32 = 1;

/// Default sidecar directory name relative to workspace root.
pub const DEFAULT_SIDECAR_DIR: &str = ".markymark";

// ---------------------------------------------------------------------------
// Sidecar data types
// ---------------------------------------------------------------------------

/// Enrichment metadata for a single document, stored as a JSON sidecar file.
///
/// Content hash enables invalidation: when the source document changes,
/// the hash won't match and the sidecar is considered stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSidecar {
    /// Schema version for forward compatibility.
    pub version: u32,
    /// SHA-256 hex digest of the source document content.
    pub content_hash: String,
    /// Model identifier that generated these summaries.
    pub model_id: String,
    /// Optional document-level summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_summary: Option<String>,
    /// Per-section summaries keyed by heading slug path.
    pub sections: Vec<SectionSummary>,
}

/// Summary for a single heading/section in a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSummary {
    /// Heading slug (unique within document, e.g. "getting-started").
    pub slug: String,
    /// Full heading path for context (e.g. "Overview > Getting Started").
    pub heading_path: String,
    /// Heading level (1-6).
    pub level: u8,
    /// LLM-generated summary of this section's content.
    pub summary: String,
}

impl DocumentSidecar {
    /// Create a new sidecar with the given content hash and model.
    pub fn new(content_hash: String, model_id: String) -> Self {
        Self {
            version: SIDECAR_VERSION,
            content_hash,
            model_id,
            document_summary: None,
            sections: Vec::new(),
        }
    }

    /// Check if this sidecar is stale (content hash doesn't match).
    pub fn is_stale(&self, current_hash: &str) -> bool {
        self.content_hash != current_hash
    }
}

/// Compute a SHA-256 hex digest of the given content.
pub fn content_hash(content: &[u8]) -> String {
    // Simple SHA-256 implementation using Rust's built-in — no external dep needed.
    // We use a minimal hand-rolled SHA-256 to avoid pulling in a crypto crate
    // just for content hashing.
    sha256_hex(content)
}

/// Derive the sidecar file path for a document.
///
/// Given a workspace root and a document path relative to that root,
/// returns the path where the sidecar JSON should be stored.
///
/// Example: root=/workspace, relative=docs/guide.md → /workspace/.markymark/docs/guide.md.json
pub fn sidecar_path(
    sidecar_dir: &std::path::Path,
    relative_doc_path: &std::path::Path,
) -> std::path::PathBuf {
    let mut path = sidecar_dir.join(relative_doc_path);
    let mut name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    name.push_str(".json");
    path.set_file_name(name);
    path
}

// ---------------------------------------------------------------------------
// Minimal SHA-256 (no external dependency)
// ---------------------------------------------------------------------------

fn sha256_hex(data: &[u8]) -> String {
    let hash = sha256(data);
    let mut hex = String::with_capacity(64);
    for byte in &hash {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Pre-processing: padding
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        result[4 * i..4 * i + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash(b"hello world");
        let h2 = content_hash(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_content_hash_known_vector() {
        // SHA-256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let hash = content_hash(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_content_hash_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = content_hash(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_content_hash_different_inputs() {
        let h1 = content_hash(b"hello");
        let h2 = content_hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_sidecar_new() {
        let sidecar = DocumentSidecar::new("abc123".to_string(), "claude-haiku".to_string());
        assert_eq!(sidecar.version, SIDECAR_VERSION);
        assert_eq!(sidecar.content_hash, "abc123");
        assert_eq!(sidecar.model_id, "claude-haiku");
        assert!(sidecar.document_summary.is_none());
        assert!(sidecar.sections.is_empty());
    }

    #[test]
    fn test_sidecar_is_stale() {
        let sidecar = DocumentSidecar::new("hash_v1".to_string(), "test".to_string());
        assert!(!sidecar.is_stale("hash_v1"));
        assert!(sidecar.is_stale("hash_v2"));
    }

    #[test]
    fn test_sidecar_serialization_roundtrip() {
        let sidecar = DocumentSidecar {
            version: SIDECAR_VERSION,
            content_hash: "abc123".to_string(),
            model_id: "claude-haiku".to_string(),
            document_summary: Some("A document about testing".to_string()),
            sections: vec![
                SectionSummary {
                    slug: "intro".to_string(),
                    heading_path: "Introduction".to_string(),
                    level: 1,
                    summary: "Introduces the topic".to_string(),
                },
                SectionSummary {
                    slug: "details".to_string(),
                    heading_path: "Introduction > Details".to_string(),
                    level: 2,
                    summary: "Provides detailed info".to_string(),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&sidecar).unwrap();
        let deserialized: DocumentSidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(sidecar, deserialized);
    }

    #[test]
    fn test_sidecar_serialization_skips_none_summary() {
        let sidecar = DocumentSidecar::new("hash".to_string(), "model".to_string());
        let json = serde_json::to_string(&sidecar).unwrap();
        assert!(!json.contains("document_summary"));
    }

    #[test]
    fn test_sidecar_path_simple() {
        let dir = std::path::Path::new("/workspace/.markymark");
        let doc = std::path::Path::new("docs/guide.md");
        let path = sidecar_path(dir, doc);
        assert_eq!(
            path,
            std::path::PathBuf::from("/workspace/.markymark/docs/guide.md.json")
        );
    }

    #[test]
    fn test_sidecar_path_nested() {
        let dir = std::path::Path::new("/workspace/.markymark");
        let doc = std::path::Path::new("deep/nested/file.md");
        let path = sidecar_path(dir, doc);
        assert_eq!(
            path,
            std::path::PathBuf::from("/workspace/.markymark/deep/nested/file.md.json")
        );
    }

    #[test]
    fn test_sidecar_path_root_file() {
        let dir = std::path::Path::new("/workspace/.markymark");
        let doc = std::path::Path::new("README.md");
        let path = sidecar_path(dir, doc);
        assert_eq!(
            path,
            std::path::PathBuf::from("/workspace/.markymark/README.md.json")
        );
    }
}
