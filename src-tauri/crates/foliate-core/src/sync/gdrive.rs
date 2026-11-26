use crate::sync::crdt::SyncDocument;
use crate::{CoreError, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;
use std::io::{Read, Write};

pub const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
pub const DRIVE_UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
pub const APP_DATA_FOLDER: &str = "appDataFolder";
pub const SYNC_FILE_NAME: &str = "foliate_sync_v1.json.gz";
const MAX_SYNC_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Download {
    NotModified,
    New {
        bytes: Vec<u8>,
        etag: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<FileJson>,
}

#[derive(Debug, Deserialize)]
struct FileJson {
    id: String,
    name: String,
}

/// Minimal Google Drive REST v3 client scoped to `appDataFolder`.
#[derive(Clone)]
pub struct DriveClient {
    http: reqwest::Client,
    access_token: String,
}

impl DriveClient {
    pub fn new(access_token: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("wolidate/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            access_token: access_token.into(),
        })
    }

    pub fn with_client(http: reqwest::Client, access_token: impl Into<String>) -> Self {
        Self {
            http,
            access_token: access_token.into(),
        }
    }

    async fn check(response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().await.unwrap_or_default();
        let body = body.chars().take(500).collect::<String>();
        Err(CoreError::Invalid(format!(
            "Google Drive API returned {status}: {body}"
        )))
    }

    /// Find the single sync file inside the hidden app data folder.
    pub async fn find_sync_file(&self) -> Result<Option<DriveFile>> {
        let response = self
            .http
            .get(format!("{DRIVE_API}/files"))
            .bearer_auth(&self.access_token)
            .query(&[
                ("spaces", APP_DATA_FOLDER),
                ("q", "name = 'foliate_sync_v1.json.gz' and trashed = false"),
                ("fields", "files(id,name,modifiedTime)"),
                ("pageSize", "10"),
            ])
            .send()
            .await?;
        let response = Self::check(response).await?;
        let list: FileList = response.json().await?;
        Ok(list.files.into_iter().next().map(|file| DriveFile {
            id: file.id,
            name: file.name,
            etag: None,
        }))
    }

    /// Download the sync document. `etag` enables conditional requests.
    pub async fn download(&self, file_id: &str, etag: Option<&str>) -> Result<Download> {
        let mut request = self
            .http
            .get(format!("{DRIVE_API}/files/{file_id}"))
            .bearer_auth(&self.access_token)
            .query(&[("alt", "media")]);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Download::NotModified);
        }
        let response = Self::check(response).await?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response.bytes().await?.to_vec();
        Ok(Download::New { bytes, etag })
    }

    /// Create the sync file as a new `appDataFolder` child.
    pub async fn create(&self, bytes: &[u8]) -> Result<DriveFile> {
        let metadata = serde_json::json!({
            "name": SYNC_FILE_NAME,
            "parents": [APP_DATA_FOLDER],
            "mimeType": "application/gzip",
        });
        let (content_type, body) = multipart_body(&metadata.to_string(), bytes);
        let response = self
            .http
            .post(format!(
                "{DRIVE_UPLOAD_API}/files?uploadType=multipart&fields=id,name"
            ))
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        let response = Self::check(response).await?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let file: FileJson = response.json().await?;
        Ok(DriveFile {
            id: file.id,
            name: file.name,
            etag,
        })
    }

    /// Replace the contents of an existing sync file.
    pub async fn update(&self, file_id: &str, bytes: &[u8]) -> Result<DriveFile> {
        let response = self
            .http
            .patch(format!(
                "{DRIVE_UPLOAD_API}/files/{file_id}?uploadType=media&fields=id,name"
            ))
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/gzip")
            .body(bytes.to_vec())
            .send()
            .await?;
        let response = Self::check(response).await?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let file: FileJson = response.json().await?;
        Ok(DriveFile {
            id: file.id,
            name: file.name,
            etag,
        })
    }
}

fn multipart_body(metadata_json: &str, media: &[u8]) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "foliate-sync-boundary-7d1f";
    let content_type = format!("multipart/related; boundary={BOUNDARY}");
    let mut body = Vec::with_capacity(metadata_json.len() + media.len() + 256);
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    body.extend_from_slice(metadata_json.as_bytes());
    body.extend_from_slice(format!("\r\n--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/gzip\r\n\r\n");
    body.extend_from_slice(media);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    (content_type, body)
}

/// Serialize a document as gzipped JSON (the canonical Drive payload).
pub fn encode_document(document: &SyncDocument) -> Result<Vec<u8>> {
    document.validate()?;
    let json = serde_json::to_vec(document)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    Ok(encoder.finish()?)
}

/// Parse gzipped JSON with a decompressed-size cap.
pub fn decode_document(bytes: &[u8]) -> Result<SyncDocument> {
    let mut decoder = GzDecoder::new(bytes).take(MAX_SYNC_BYTES + 1);
    let mut json = Vec::new();
    decoder.read_to_end(&mut json)?;
    if json.len() as u64 > MAX_SYNC_BYTES {
        return Err(CoreError::Invalid(
            "sync document exceeds the decompressed size cap".into(),
        ));
    }
    let document: SyncDocument = serde_json::from_slice(&json)?;
    document.validate()?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::crdt::{CollectionRecord, ProgressRecord};

    #[test]
    fn document_round_trips_through_gzip() {
        let mut document = SyncDocument::default();
        document.progress.insert(
            "book".into(),
            ProgressRecord {
                cfi: "epubcfi(/6/2)".into(),
                fraction: 0.5,
                updated_at_ms: 10,
                device_id: "win-a".into(),
            },
        );
        document.collections.insert(
            "c1".into(),
            CollectionRecord {
                name: "Systems".into(),
                updated_at_ms: 11,
                deleted_at_ms: None,
            },
        );
        let encoded = encode_document(&document).unwrap();
        assert_eq!(&encoded[..2], &[0x1f, 0x8b]);
        let decoded = decode_document(&encoded).unwrap();
        assert_eq!(decoded, document);
    }

    #[test]
    fn multipart_body_has_both_parts() {
        let (content_type, body) = multipart_body(r#"{"name":"x"}"#, b"payload");
        assert!(content_type.starts_with("multipart/related; boundary="));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("Content-Type: application/json"));
        assert!(text.contains(r#"{"name":"x"}"#));
        assert!(text.contains("Content-Type: application/gzip"));
        assert!(text.contains("payload"));
    }

    #[test]
    fn rejects_a_document_with_a_future_schema() {
        let json = br#"{"schema_version":99,"exported_at_ms":0,"progress":{},"annotations":{},"collections":{},"book_collections":{}}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json).unwrap();
        let encoded = encoder.finish().unwrap();
        assert!(matches!(
            decode_document(&encoded),
            Err(CoreError::SyncSchema(99))
        ));
    }
}
