//! Pure domain logic for wolidate v0.1.
//!
//! Everything in this crate is platform-neutral and testable with
//! `cargo test -p foliate-core`. The Tauri shell owns only file-descriptor
//! resolution, IPC, keyring access, and OAuth redirects.

pub mod archive;
pub mod db;
pub mod hash;
pub mod metadata;
pub mod source;
pub mod sync;

use serde::{Deserialize, Serialize};

pub use source::BookSource;

/// Errors shared across the core crate.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("SQLite error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("XML error: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("sync schema {0} is not supported")]
    SyncSchema(u32),
}

pub type Result<T> = std::result::Result<T, CoreError>;

/// Supported book containers. Adding a variant requires a matching SQLite
/// `format` value and a loader in the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Epub,
    Mobi,
    Azw3,
    Fb2,
    Cbz,
    Pdf,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Epub => "epub",
            Self::Mobi => "mobi",
            Self::Azw3 => "azw3",
            Self::Fb2 => "fb2",
            Self::Cbz => "cbz",
            Self::Pdf => "pdf",
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "epub" => Some(Self::Epub),
            "mobi" => Some(Self::Mobi),
            "azw" | "azw3" => Some(Self::Azw3),
            "fb2" => Some(Self::Fb2),
            "cbz" => Some(Self::Cbz),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Detect from the file header. Extension detection wins when present
    /// because `.azw3` and `.mobi` share the same container.
    pub fn detect(name: &str, magic: &[u8]) -> Option<Self> {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
        {
            return Some(ext);
        }
        if magic.starts_with(b"%PDF-") {
            return Some(Self::Pdf);
        }
        if magic.starts_with(b"PK\x03\x04") {
            return Some(Self::Epub);
        }
        if magic.len() >= 68 && &magic[60..68] == b"BOOKMOBI" {
            return Some(Self::Mobi);
        }
        None
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Unix epoch milliseconds, UTC. Never use a locale-dependent clock.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A short, human-readable device id stored in `device_meta`.
pub fn new_device_id(prefix: &str) -> String {
    let id = uuid::Uuid::now_v7().simple().to_string();
    format!("{prefix}-{}", &id[..8])
}

/// UUIDv7 identifier for annotations and collections.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats_from_extension_and_magic() {
        assert_eq!(Format::detect("book.EPUB", b""), Some(Format::Epub));
        assert_eq!(Format::detect("book.azw3", b"BOOKMOBI"), Some(Format::Azw3));
        assert_eq!(Format::detect("book", b"%PDF-1.7"), Some(Format::Pdf));
        assert_eq!(
            Format::detect("book", b"PK\x03\x04rest"),
            Some(Format::Epub)
        );
        assert_eq!(Format::detect("book", b"nope"), None);
        assert_eq!(Format::detect("book", b"short"), None);
        let mut mobi = vec![0_u8; 80];
        mobi[60..68].copy_from_slice(b"BOOKMOBI");
        assert_eq!(Format::detect("book", &mobi), Some(Format::Mobi));
    }
}
