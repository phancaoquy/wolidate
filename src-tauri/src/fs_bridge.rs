//! Platform file-descriptor resolution.
//!
//! Windows paths and Android `content://` URIs both end up as a seekable
//! `std::fs::File`, so the rest of the host and core never branch on platform.

#[cfg(target_os = "android")]
use foliate_core::CoreError;
use foliate_core::Result;
use std::fs::File;
use tauri::AppHandle;

pub struct OpenedBook {
    pub file: File,
    pub size: u64,
    pub display_name: String,
}

#[cfg(target_os = "windows")]
pub async fn open_book(_app: &AppHandle, uri: &str) -> Result<OpenedBook> {
    let path = std::path::PathBuf::from(uri);
    let file = File::open(&path)?;
    let size = file.metadata()?.len();
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| uri.to_owned());
    Ok(OpenedBook {
        file,
        size,
        display_name,
    })
}

#[cfg(target_os = "android")]
pub async fn open_book(app: &AppHandle, uri: &str) -> Result<OpenedBook> {
    use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

    let api = app.android_fs_async();
    let fs_uri = FsUri::from_uri(uri);
    let file = api
        .open_file_readable(&fs_uri)
        .await
        .map_err(|error| CoreError::Invalid(format!("content URI open failed: {error}")))?;
    let size = api
        .get_len(&fs_uri)
        .await
        .map_err(|error| CoreError::Invalid(format!("content URI size failed: {error}")))?;
    let display_name = api
        .get_name(&fs_uri)
        .await
        .map_err(|error| CoreError::Invalid(format!("content URI name failed: {error}")))?;
    Ok(OpenedBook {
        file,
        size,
        display_name,
    })
}

/// Format a protocol path segment safely for an Android/content or Windows path.
pub fn display_title(display_name: &str) -> String {
    std::path::Path::new(display_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['_', '-'], " "))
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| display_name.to_owned())
}
