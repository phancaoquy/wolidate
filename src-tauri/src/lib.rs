#[cfg(not(any(target_os = "windows", target_os = "android")))]
compile_error!("wolidate v0.1 targets Windows and Android only.");

mod fs_bridge;
mod oauth;

use foliate_core::archive::{self, ZipIndex};
use foliate_core::db::{
    self, Annotation, Book, BookCollection, Collection, NewBook, Progress, SyncState,
};
use foliate_core::metadata;
use foliate_core::{hash, now_ms, Format};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::http::header::{self, HeaderValue};
use tauri::http::{Method, Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

const MAX_RANGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;

struct AppState {
    db: Mutex<db::Connection>,
    cache_dir: PathBuf,
    device_id: String,
    pending_auth: Mutex<Option<oauth::PendingAuth>>,
    pending_open: Mutex<Vec<String>>,
    archive_indexes: Mutex<HashMap<String, Arc<ZipIndex>>>,
    http: reqwest::Client,
}

fn is_book_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "epub" | "mobi" | "azw" | "azw3" | "fb2" | "cbz" | "pdf"
            )
        })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatus {
    connected: bool,
    dirty: bool,
    last_synced_at_ms: i64,
    drive_file_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncRun {
    downloaded: bool,
    uploaded: bool,
    remote_found: bool,
    last_synced_at_ms: i64,
}

impl From<foliate_core::sync::SyncOutcome> for SyncRun {
    fn from(value: foliate_core::sync::SyncOutcome) -> Self {
        Self {
            downloaded: value.downloaded,
            uploaded: value.uploaded,
            remote_found: value.remote_found,
            last_synced_at_ms: value.last_synced_at_ms,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
struct OAuthEvent {
    ok: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationInput {
    id: Option<String>,
    book_hash: String,
    kind: String,
    cfi: String,
    color: Option<String>,
    quote_text: Option<String>,
    note_text: Option<String>,
    created_at_ms: Option<i64>,
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

// ---------------------------------------------------------------------------
// Library commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn list_books(
    state: State<'_, AppState>,
    search: Option<String>,
    collection_id: Option<String>,
) -> Result<Vec<Book>, String> {
    let connection = state.db.lock().await;
    db::list_books(&connection, search.as_deref(), collection_id.as_deref()).map_err(error_string)
}

#[tauri::command]
async fn get_book(state: State<'_, AppState>, book_hash: String) -> Result<Option<Book>, String> {
    let connection = state.db.lock().await;
    db::get_book(&connection, &book_hash).map_err(error_string)
}

#[tauri::command]
async fn import_book(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Book, String> {
    let opened = fs_bridge::open_book(&app, &path)
        .await
        .map_err(error_string)?;
    let mut file = opened.file;
    let size = opened.size;
    let mut magic = Vec::with_capacity(80);
    (&mut file)
        .take(80)
        .read_to_end(&mut magic)
        .map_err(error_string)?;
    file.seek(SeekFrom::Start(0)).map_err(error_string)?;

    let format = Format::detect(&opened.display_name, &magic)
        .ok_or_else(|| format!("unsupported book format: {}", opened.display_name))?;
    let book_hash = hash::book_hash(&mut file, size).map_err(error_string)?;

    {
        let connection = state.db.lock().await;
        if let Some(existing) = db::get_book(&connection, &book_hash).map_err(error_string)? {
            if existing.deleted_at_ms.is_none() {
                return Ok(existing);
            }
        }
    }

    file.seek(SeekFrom::Start(0)).map_err(error_string)?;
    let fallback_title = fs_bridge::display_title(&opened.display_name);
    let parsed = metadata::extract(&mut file, format, &fallback_title).map_err(error_string)?;

    let cover_rel_path = if let Some(cover) = parsed.cover.as_deref() {
        let relative = format!("covers/{book_hash}.webp");
        let absolute = state.cache_dir.join(&relative);
        match metadata::write_cover_webp(cover, &absolute, 300) {
            Ok(()) => Some(relative),
            Err(error) => {
                eprintln!("cover render failed for {book_hash}: {error}");
                None
            }
        }
    } else {
        None
    };

    let new_book = NewBook {
        book_hash: book_hash.clone(),
        source_uri: path,
        file_size: i64::try_from(size).map_err(|_| "book is too large".to_string())?,
        format: format.as_str().to_owned(),
        title: parsed.title,
        author: parsed.author,
        language: parsed.language,
        cover_rel_path,
        added_at_ms: now_ms(),
    };

    let connection = state.db.lock().await;
    db::insert_book(&connection, &new_book).map_err(error_string)?;
    db::get_book(&connection, &book_hash)
        .map_err(error_string)?
        .ok_or_else(|| "book vanished after insert".to_string())
}

#[tauri::command]
async fn delete_book(state: State<'_, AppState>, book_hash: String) -> Result<(), String> {
    let connection = state.db.lock().await;
    db::soft_delete_book(&connection, &book_hash, now_ms()).map_err(error_string)
}

#[tauri::command]
async fn get_progress(
    state: State<'_, AppState>,
    book_hash: String,
) -> Result<Option<Progress>, String> {
    let connection = state.db.lock().await;
    db::get_progress(&connection, &book_hash).map_err(error_string)
}

#[tauri::command]
async fn save_progress(
    state: State<'_, AppState>,
    book_hash: String,
    cfi: String,
    fraction: f64,
) -> Result<(), String> {
    let progress = Progress {
        book_hash,
        cfi,
        fraction: fraction.clamp(0.0, 1.0),
        updated_at_ms: now_ms(),
        device_id: state.device_id.clone(),
    };
    let connection = state.db.lock().await;
    db::save_progress(&connection, &progress).map_err(error_string)
}

#[tauri::command]
async fn list_annotations(
    state: State<'_, AppState>,
    book_hash: String,
) -> Result<Vec<Annotation>, String> {
    let connection = state.db.lock().await;
    db::list_annotations(&connection, &book_hash, false).map_err(error_string)
}

#[tauri::command]
async fn save_annotation(
    state: State<'_, AppState>,
    annotation: AnnotationInput,
) -> Result<Annotation, String> {
    if !matches!(annotation.kind.as_str(), "highlight" | "bookmark") {
        return Err(format!("invalid annotation kind: {}", annotation.kind));
    }
    let connection = state.db.lock().await;
    let now = now_ms();
    let existing = match annotation.id.as_deref() {
        Some(id) => db::get_annotation(&connection, id).map_err(error_string)?,
        None => None,
    };
    let saved = Annotation {
        id: annotation.id.unwrap_or_else(foliate_core::new_id),
        book_hash: annotation.book_hash,
        kind: annotation.kind,
        cfi: annotation.cfi,
        color: annotation.color.unwrap_or_else(|| "yellow".into()),
        quote_text: annotation.quote_text.unwrap_or_default(),
        note_text: annotation.note_text.unwrap_or_default(),
        created_at_ms: annotation
            .created_at_ms
            .or_else(|| existing.as_ref().map(|value| value.created_at_ms))
            .unwrap_or(now),
        updated_at_ms: now,
        deleted_at_ms: None,
    };
    db::upsert_annotation(&connection, &saved).map_err(error_string)?;
    Ok(saved)
}

#[tauri::command]
async fn delete_annotation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let connection = state.db.lock().await;
    db::soft_delete_annotation(&connection, &id, now_ms()).map_err(error_string)
}

#[tauri::command]
async fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>, String> {
    let connection = state.db.lock().await;
    db::list_collections(&connection).map_err(error_string)
}

#[tauri::command]
async fn create_collection(state: State<'_, AppState>, name: String) -> Result<Collection, String> {
    if name.trim().is_empty() {
        return Err("collection name is empty".into());
    }
    let connection = state.db.lock().await;
    db::create_collection(&connection, &name, now_ms()).map_err(error_string)
}

#[tauri::command]
async fn rename_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Collection, String> {
    if name.trim().is_empty() {
        return Err("collection name is empty".into());
    }
    let connection = state.db.lock().await;
    let mut collection = db::get_collection(&connection, &id)
        .map_err(error_string)?
        .ok_or_else(|| "collection not found".to_string())?;
    collection.name = name.trim().to_owned();
    collection.updated_at_ms = now_ms();
    db::upsert_collection(&connection, &collection).map_err(error_string)?;
    Ok(collection)
}

#[tauri::command]
async fn delete_collection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let connection = state.db.lock().await;
    db::soft_delete_collection(&connection, &id, now_ms()).map_err(error_string)
}

#[tauri::command]
async fn set_book_collection(
    state: State<'_, AppState>,
    book_hash: String,
    collection_id: String,
    added: bool,
) -> Result<(), String> {
    let connection = state.db.lock().await;
    db::set_book_collection(&connection, &book_hash, &collection_id, added, now_ms())
        .map_err(error_string)
}

#[tauri::command]
async fn list_book_collections(state: State<'_, AppState>) -> Result<Vec<BookCollection>, String> {
    let connection = state.db.lock().await;
    let mut rows = db::list_book_collections(&connection).map_err(error_string)?;
    rows.retain(|row| row.deleted_at_ms.is_none());
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Sync + OAuth commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let connected = oauth::load_tokens()
        .map(|tokens| tokens.is_some())
        .unwrap_or(false);
    let connection = state.db.lock().await;
    let sync: SyncState = db::get_sync_state(&connection).map_err(error_string)?;
    Ok(SyncStatus {
        connected,
        dirty: sync.dirty,
        last_synced_at_ms: sync.last_synced_at_ms,
        drive_file_id: sync.drive_file_id,
    })
}

#[tauri::command]
async fn sync_now(state: State<'_, AppState>) -> Result<SyncRun, String> {
    let access_token = oauth::fresh_access_token(&state.http).await?;
    let client = foliate_core::sync::DriveClient::new(access_token).map_err(error_string)?;
    let mut connection = state.db.lock().await;
    let outcome = foliate_core::sync::sync_once(&mut connection, &client, now_ms())
        .await
        .map_err(error_string)?;
    Ok(outcome.into())
}

#[tauri::command]
async fn google_oauth_start(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let (pending, url) = oauth::begin()?;
    *state.pending_auth.lock().await = Some(pending);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn google_oauth_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    *state.pending_auth.lock().await = None;
    oauth::clear_tokens()
}

#[tauri::command]
fn device_id(state: State<'_, AppState>) -> String {
    state.device_id.clone()
}

#[tauri::command]
async fn take_pending_open(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut pending = state.pending_open.lock().await;
    Ok(std::mem::take(&mut *pending))
}

#[tauri::command]
fn set_window_theme(
    window: tauri::WebviewWindow,
    theme: String,
    caption: String,
    text: String,
) -> Result<(), String> {
    let theme = match theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" | "sepia" => Some(tauri::Theme::Light),
        _ => None,
    };
    window.set_theme(theme).map_err(error_string)?;
    #[cfg(windows)]
    apply_caption_colors(&window, &caption, &text);
    Ok(())
}

#[cfg(windows)]
fn apply_caption_colors(window: &tauri::WebviewWindow, caption: &str, text: &str) {
    use std::ffi::c_void;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    };

    fn colorref(hex: &str) -> Option<u32> {
        let value = hex.trim().strip_prefix('#')?;
        if value.len() != 6 {
            return None;
        }
        let r = u32::from_str_radix(&value[0..2], 16).ok()?;
        let g = u32::from_str_radix(&value[2..4], 16).ok()?;
        let b = u32::from_str_radix(&value[4..6], 16).ok()?;
        Some(r | (g << 8) | (b << 16))
    }

    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = hwnd.0;
    if let Some(color) = colorref(caption) {
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR as u32,
                &color as *const u32 as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    if let Some(color) = colorref(text) {
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR as u32,
                &color as *const u32 as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Deep links
// ---------------------------------------------------------------------------

fn emit_oauth(app: &AppHandle, ok: bool, message: impl Into<String>) {
    let _ = app.emit(
        "oauth-complete",
        OAuthEvent {
            ok,
            message: message.into(),
        },
    );
}

fn handle_deep_link(app: AppHandle, url: url::Url) {
    if url.scheme() != "wolidate" {
        return;
    }
    let is_oauth = url.host_str() == Some("oauth2redirect") || url.path() == "/oauth2redirect";
    if !is_oauth {
        return;
    }
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        emit_oauth(&app, false, format!("Google authorization failed: {error}"));
        return;
    }
    let Some(code) = params.get("code").cloned() else {
        emit_oauth(&app, false, "Google authorization returned no code");
        return;
    };
    let Some(callback_state) = params.get("state").cloned() else {
        emit_oauth(&app, false, "Google authorization returned no state");
        return;
    };

    tauri::async_runtime::spawn(async move {
        let (pending, http) = {
            let state = app.state::<AppState>();
            let mut guard = state.pending_auth.lock().await;
            (guard.take(), state.http.clone())
        };
        let Some(pending) = pending else {
            emit_oauth(&app, false, "No Google authorization is pending");
            return;
        };
        if pending.state != callback_state {
            emit_oauth(&app, false, "Google authorization state mismatch");
            return;
        }
        match oauth::exchange_code(&http, &code, &pending).await {
            Ok(tokens) => match oauth::save_tokens(&tokens) {
                Ok(()) => emit_oauth(&app, true, "Google Drive connected"),
                Err(error) => emit_oauth(&app, false, error),
            },
            Err(error) => emit_oauth(&app, false, error),
        }
    });
}

// ---------------------------------------------------------------------------
// Custom protocol: http://book.localhost/{book_hash}/{entry}
// ---------------------------------------------------------------------------

fn header_value(value: impl std::fmt::Display) -> HeaderValue {
    HeaderValue::from_str(&value.to_string()).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn response_builder(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Range, Content-Type, If-None-Match",
        )
        .header(
            "Access-Control-Expose-Headers",
            "Content-Length, Content-Range, Accept-Ranges",
        )
}

fn build_response(builder: tauri::http::response::Builder, body: Vec<u8>) -> Response<Vec<u8>> {
    builder
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn text_response(status: StatusCode, message: impl Into<String>) -> Response<Vec<u8>> {
    let message = message.into();
    build_response(
        response_builder(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CONTENT_LENGTH, header_value(message.len())),
        message.into_bytes(),
    )
}

fn range_not_satisfiable(size: u64) -> Response<Vec<u8>> {
    build_response(
        response_builder(StatusCode::RANGE_NOT_SATISFIABLE)
            .header("Accept-Ranges", "bytes")
            .header("Content-Range", header_value(format!("bytes */{size}")))
            .header(header::CONTENT_LENGTH, header_value(0)),
        Vec::new(),
    )
}

fn book_content_type(format: &str) -> &'static str {
    match format {
        "epub" => "application/epub+zip",
        "mobi" | "azw3" => "application/x-mobipocket-ebook",
        "fb2" => "application/x-fictionbook+xml",
        "cbz" => "application/vnd.comicbook+zip",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

async fn ensure_index(
    app: &AppHandle,
    state: &AppState,
    book: &Book,
) -> Result<Arc<ZipIndex>, String> {
    if let Some(index) = state
        .archive_indexes
        .lock()
        .await
        .get(&book.book_hash)
        .cloned()
    {
        return Ok(index);
    }
    let mut opened = fs_bridge::open_book(app, &book.source_uri)
        .await
        .map_err(error_string)?;
    let index = Arc::new(archive::index_zip(&mut opened.file).map_err(error_string)?);
    state
        .archive_indexes
        .lock()
        .await
        .insert(book.book_hash.clone(), Arc::clone(&index));
    Ok(index)
}

async fn serve_raw(
    app: &AppHandle,
    book: &Book,
    range_header: Option<&str>,
    head_only: bool,
) -> Response<Vec<u8>> {
    let mut opened = match fs_bridge::open_book(app, &book.source_uri).await {
        Ok(opened) => opened,
        Err(error) => return text_response(StatusCode::NOT_FOUND, error.to_string()),
    };
    let size = opened.size;
    let content_type = book_content_type(&book.format);

    // A whole-book 200 response would buffer the entire archive. The reader
    // blob always asks for ranges, so reject anything else.
    let Some(range_header) = range_header else {
        return range_not_satisfiable(size);
    };
    let range = match archive::parse_range_header(range_header, size) {
        Ok(Some(range)) => range,
        Ok(None) | Err(_) => return range_not_satisfiable(size),
    };
    if range.len() > MAX_RANGE_BYTES {
        return range_not_satisfiable(size);
    }

    let builder = response_builder(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, header_value(range.len()))
        .header(
            header::CONTENT_RANGE,
            header_value(format!("bytes {}-{}/{}", range.start, range.end, size)),
        )
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store");

    if head_only {
        return build_response(builder, Vec::new());
    }
    match archive::read_range(&mut opened.file, range) {
        Ok(bytes) => build_response(builder, bytes),
        Err(error) => text_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn serve_entry(
    app: &AppHandle,
    state: &AppState,
    book: &Book,
    entry: &str,
    range_header: Option<&str>,
    head_only: bool,
) -> Response<Vec<u8>> {
    let index = match ensure_index(app, state, book).await {
        Ok(index) => index,
        Err(error) => return text_response(StatusCode::BAD_REQUEST, error),
    };
    let Some(meta) = index.get(entry) else {
        return text_response(StatusCode::NOT_FOUND, format!("entry not found: {entry}"));
    };
    if meta.is_dir {
        return text_response(
            StatusCode::NOT_FOUND,
            format!("entry is a directory: {entry}"),
        );
    }
    if meta.uncompressed_size > MAX_ENTRY_BYTES {
        return text_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("entry is larger than {MAX_ENTRY_BYTES} bytes"),
        );
    }

    let total = meta.uncompressed_size;
    let content_type = archive::content_type(entry);
    let mut builder = response_builder(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store")
        .header(header::CONTENT_LENGTH, header_value(total));

    let range = match range_header {
        Some(value) => match archive::parse_range_header(value, total) {
            Ok(Some(range)) => {
                if range.len() > MAX_RANGE_BYTES {
                    return range_not_satisfiable(total);
                }
                Some(range)
            }
            Ok(None) | Err(_) => return range_not_satisfiable(total),
        },
        None => None,
    };

    if let Some(range) = range {
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(
                header::CONTENT_RANGE,
                header_value(format!("bytes {}-{}/{}", range.start, range.end, total)),
            )
            .header(header::CONTENT_LENGTH, header_value(range.len()));
    }

    if head_only {
        return build_response(builder, Vec::new());
    }

    let mut opened = match fs_bridge::open_book(app, &book.source_uri).await {
        Ok(opened) => opened,
        Err(error) => return text_response(StatusCode::NOT_FOUND, error.to_string()),
    };
    let bytes = match archive::read_entry_bytes(&mut opened.file, entry, MAX_ENTRY_BYTES) {
        Ok(bytes) => bytes,
        Err(error) => return text_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let bytes = match range {
        Some(range) => {
            let start = usize::try_from(range.start).unwrap_or(0);
            let end = usize::try_from(range.end.saturating_add(1)).unwrap_or(bytes.len());
            bytes
                .get(start..end.min(bytes.len()))
                .unwrap_or_default()
                .to_vec()
        }
        None => bytes,
    };
    build_response(builder, bytes)
}

fn serve_cover(state: &AppState, book: &Book) -> Response<Vec<u8>> {
    let Some(relative) = book.cover_rel_path.as_deref() else {
        return text_response(StatusCode::NOT_FOUND, "no cover");
    };
    if relative.contains("..") {
        return text_response(StatusCode::BAD_REQUEST, "invalid cover path");
    }
    match std::fs::read(state.cache_dir.join(relative)) {
        Ok(bytes) => build_response(
            response_builder(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/webp")
                .header(header::CONTENT_LENGTH, header_value(bytes.len()))
                .header("Cache-Control", "private, max-age=86400"),
            bytes,
        ),
        Err(_) => text_response(StatusCode::NOT_FOUND, "no cover"),
    }
}

async fn handle_book_request(app: &AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let method = request.method().clone();
    if method == Method::OPTIONS {
        return build_response(response_builder(StatusCode::NO_CONTENT), Vec::new());
    }
    if method != Method::GET && method != Method::HEAD {
        return build_response(
            response_builder(StatusCode::METHOD_NOT_ALLOWED)
                .header(header::ALLOW, "GET, HEAD, OPTIONS"),
            Vec::new(),
        );
    }
    let head_only = method == Method::HEAD;

    let path = request.uri().path().to_owned();
    let segments: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| percent_decode_str(segment).decode_utf8_lossy().into_owned())
        .collect();
    if segments.is_empty() {
        return text_response(StatusCode::NOT_FOUND, "missing book hash");
    }
    if segments[0] == "__health" {
        return build_response(
            response_builder(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .header(header::CONTENT_LENGTH, header_value(2)),
            b"ok".to_vec(),
        );
    }

    let state = app.state::<AppState>();
    let book = {
        let connection = state.db.lock().await;
        match db::get_book(&connection, &segments[0]) {
            Ok(Some(book)) => book,
            Ok(None) => return text_response(StatusCode::NOT_FOUND, "book not found"),
            Err(error) => {
                return text_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            }
        }
    };

    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    if segments.len() == 1 {
        return serve_raw(app, &book, range_header.as_deref(), head_only).await;
    }
    if segments.len() == 2 && segments[1] == "__cover" {
        return serve_cover(state.inner(), &book);
    }
    let entry = segments[1..].join("/");
    serve_entry(
        app,
        state.inner(),
        &book,
        &entry,
        range_header.as_deref(),
        head_only,
    )
    .await
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(windows)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let paths: Vec<String> = argv
                .into_iter()
                .skip(1)
                .filter(|path| is_book_path(path))
                .collect();
            if !paths.is_empty() {
                let _ = app.emit("open-file", paths);
            }
        }));
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_android_fs::builder().build());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri_plugin_deep_link::DeepLinkExt;

            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&cache_dir)?;
            let connection = db::open(&data_dir.join("foliate.sqlite3"))?;
            let prefix = if cfg!(target_os = "android") {
                "android"
            } else {
                "win"
            };
            let device_id = db::ensure_device_id(&connection, prefix)?;
            let pending_open: Vec<String> = std::env::args()
                .skip(1)
                .filter(|path| is_book_path(path))
                .collect();

            #[cfg(windows)]
            {
                let _ = app.deep_link().register_all();
            }

            app.manage(AppState {
                db: Mutex::new(connection),
                cache_dir,
                device_id,
                pending_auth: Mutex::new(None),
                pending_open: Mutex::new(pending_open),
                archive_indexes: Mutex::new(HashMap::new()),
                http: reqwest::Client::builder()
                    .user_agent(concat!("wolidate/", env!("CARGO_PKG_VERSION")))
                    .build()?,
            });

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_deep_link(handle.clone(), url);
                }
            });
            if let Some(urls) = app.deep_link().get_current()? {
                for url in urls {
                    handle_deep_link(app.handle().clone(), url);
                }
            }
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("book", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let response = handle_book_request(&app, request).await;
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            list_books,
            get_book,
            import_book,
            delete_book,
            get_progress,
            save_progress,
            list_annotations,
            save_annotation,
            delete_annotation,
            list_collections,
            create_collection,
            rename_collection,
            delete_collection,
            set_book_collection,
            list_book_collections,
            sync_status,
            sync_now,
            google_oauth_start,
            google_oauth_disconnect,
            device_id,
            take_pending_open,
            set_window_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running wolidate");
}
