use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, position: Position) -> bool {
        if position.line < self.start.line || position.line > self.end.line {
            return false;
        }

        if position.line == self.start.line && position.character < self.start.character {
            return false;
        }

        if position.line == self.end.line && position.character > self.end.character {
            return false;
        }

        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentUri {
    raw: String,
}

impl DocumentUri {
    pub fn new(uri: impl Into<String>) -> Result<Self, DocumentUriError> {
        let raw = uri.into();
        if raw.split_once("://").is_none() {
            return Err(DocumentUriError::MissingScheme);
        }
        Ok(Self { raw })
    }

    pub fn from_file_path(path: &Path) -> Self {
        let path = path.to_string_lossy();
        let encoded = percent_encode_path(&path);
        Self {
            raw: format!("file://{encoded}"),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn to_file_path(&self) -> Option<PathBuf> {
        let path = self.raw.strip_prefix("file://")?;
        let decoded = percent_decode_path(path);
        Some(PathBuf::from(decoded))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentUriError {
    MissingScheme,
}

impl fmt::Display for DocumentUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScheme => f.write_str("URI is missing a scheme"),
        }
    }
}

impl std::error::Error for DocumentUriError {}

fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(byte as char)
            }
            _ => {
                out.push('%');
                out.push(hex_digit(byte >> 4));
                out.push(hex_digit(byte & 0x0f));
            }
        }
    }
    out
}

fn percent_decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = String::with_capacity(path.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = from_hex_digit(bytes[i + 1]);
            let lo = from_hex_digit(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4 | lo) as char);
                i += 3;
                continue;
            }
        }

        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'A' + (value - 10)) as char,
    }
}

fn from_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
