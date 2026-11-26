//! Google Drive `appDataFolder` synchronization.
//!
//! The sync document contains reading status only: progress, annotations,
//! collections, and membership. Book files never leave the device.

pub mod crdt;
pub mod gdrive;

pub use crdt::{
    merge_documents, prune_tombstones, AnnotationRecord, BookCollectionRecord, CollectionRecord,
    HasUpdatedAt, ProgressRecord, SyncDocument, SYNC_SCHEMA_VERSION, TOMBSTONE_MAX_AGE_MS,
};
pub use gdrive::{
    decode_document, encode_document, Download, DriveClient, DriveFile, SYNC_FILE_NAME,
};

use crate::{db, CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension};

pub const META_DRIVE_ETAG: &str = "drive_etag";
pub const META_LAST_REMOTE: &str = "last_remote_doc";

/// Read every locally known status row into the canonical document shape.
pub fn collect_document(connection: &Connection, now_ms: i64) -> Result<SyncDocument> {
    let mut document = SyncDocument {
        schema_version: SYNC_SCHEMA_VERSION,
        exported_at_ms: now_ms,
        ..Default::default()
    };

    let mut stmt = connection.prepare(
        "SELECT book_hash, cfi, fraction, updated_at_ms, device_id FROM reading_progress",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ProgressRecord {
                cfi: row.get(1)?,
                fraction: row.get(2)?,
                updated_at_ms: row.get(3)?,
                device_id: row.get(4)?,
            },
        ))
    })?;
    for row in rows {
        let (hash, record) = row?;
        document.progress.insert(hash, record);
    }

    let mut stmt = connection.prepare(
        "SELECT id, book_hash, kind, cfi, color, quote_text, note_text,
                created_at_ms, updated_at_ms, deleted_at_ms
         FROM annotations",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            AnnotationRecord {
                book_hash: row.get(1)?,
                kind: row.get(2)?,
                cfi: row.get(3)?,
                color: row.get(4)?,
                quote_text: row.get(5)?,
                note_text: row.get(6)?,
                created_at_ms: row.get(7)?,
                updated_at_ms: row.get(8)?,
                deleted_at_ms: row.get(9)?,
            },
        ))
    })?;
    for row in rows {
        let (id, record) = row?;
        document.annotations.insert(id, record);
    }

    let mut stmt =
        connection.prepare("SELECT id, name, updated_at_ms, deleted_at_ms FROM collections")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            CollectionRecord {
                name: row.get(1)?,
                updated_at_ms: row.get(2)?,
                deleted_at_ms: row.get(3)?,
            },
        ))
    })?;
    for row in rows {
        let (id, record) = row?;
        document.collections.insert(id, record);
    }

    let mut stmt = connection.prepare(
        "SELECT book_hash, collection_id, updated_at_ms, deleted_at_ms FROM book_collections",
    )?;
    let rows = stmt.query_map([], |row| {
        let book_hash: String = row.get(0)?;
        let collection_id: String = row.get(1)?;
        let key = format!("{book_hash}:{collection_id}");
        Ok((
            key,
            BookCollectionRecord {
                book_hash,
                collection_id,
                updated_at_ms: row.get(2)?,
                deleted_at_ms: row.get(3)?,
            },
        ))
    })?;
    for row in rows {
        let (key, record) = row?;
        document.book_collections.insert(key, record);
    }

    Ok(document)
}

/// Merge `remote` into local state and write the merged result back.
///
/// Rows that reference a book which is not imported on this device are kept
/// in the returned document (so a later upload cannot erase them) but are not
/// written to SQLite, because the local schema intentionally has no orphan
/// status rows.
pub fn merge_and_apply(
    connection: &mut Connection,
    remote: &SyncDocument,
    now_ms: i64,
) -> Result<SyncDocument> {
    let local = collect_document(connection, now_ms)?;
    let merged = merge_documents(&local, remote)?;

    let tx = connection.transaction()?;
    for (hash, record) in &merged.progress {
        if book_exists(&tx, hash)? {
            tx.execute(
                "INSERT INTO reading_progress (book_hash, cfi, fraction, updated_at_ms, device_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(book_hash) DO UPDATE SET
                    cfi = excluded.cfi,
                    fraction = excluded.fraction,
                    updated_at_ms = excluded.updated_at_ms,
                    device_id = excluded.device_id",
                params![
                    hash,
                    record.cfi,
                    record.fraction,
                    record.updated_at_ms,
                    record.device_id
                ],
            )?;
        }
    }

    for (id, record) in &merged.annotations {
        if book_exists(&tx, &record.book_hash)? {
            tx.execute(
                "INSERT INTO annotations (
                    id, book_hash, kind, cfi, color, quote_text, note_text,
                    created_at_ms, updated_at_ms, deleted_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    book_hash = excluded.book_hash,
                    kind = excluded.kind,
                    cfi = excluded.cfi,
                    color = excluded.color,
                    quote_text = excluded.quote_text,
                    note_text = excluded.note_text,
                    created_at_ms = excluded.created_at_ms,
                    updated_at_ms = excluded.updated_at_ms,
                    deleted_at_ms = excluded.deleted_at_ms",
                params![
                    id,
                    record.book_hash,
                    record.kind,
                    record.cfi,
                    record.color,
                    record.quote_text,
                    record.note_text,
                    record.created_at_ms,
                    record.updated_at_ms,
                    record.deleted_at_ms
                ],
            )?;
        }
    }

    for (id, record) in &merged.collections {
        tx.execute(
            "INSERT INTO collections (id, name, updated_at_ms, deleted_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                updated_at_ms = excluded.updated_at_ms,
                deleted_at_ms = excluded.deleted_at_ms",
            params![id, record.name, record.updated_at_ms, record.deleted_at_ms],
        )?;
    }

    for record in merged.book_collections.values() {
        if book_exists(&tx, &record.book_hash)? && collection_exists(&tx, &record.collection_id)? {
            tx.execute(
                "INSERT INTO book_collections (
                    book_hash, collection_id, updated_at_ms, deleted_at_ms
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(book_hash, collection_id) DO UPDATE SET
                    updated_at_ms = excluded.updated_at_ms,
                    deleted_at_ms = excluded.deleted_at_ms",
                params![
                    record.book_hash,
                    record.collection_id,
                    record.updated_at_ms,
                    record.deleted_at_ms
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(merged)
}

fn book_exists(connection: &Connection, book_hash: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM books WHERE book_hash = ?1",
            params![book_hash],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(Into::into)
}

fn collection_exists(connection: &Connection, collection_id: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM collections WHERE id = ?1",
            params![collection_id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(Into::into)
}

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub downloaded: bool,
    pub uploaded: bool,
    pub remote_found: bool,
    pub last_synced_at_ms: i64,
}

/// One full sync pass: find, conditionally download, merge, upload if needed.
///
/// Local mutations made by the remote merge intentionally do not set the dirty
/// flag; only real local edits do. Otherwise every sync would upload forever.
pub async fn sync_once(
    connection: &mut Connection,
    client: &DriveClient,
    now_ms: i64,
) -> Result<SyncOutcome> {
    let remote_file = client.find_sync_file().await?;
    let remote_found = remote_file.is_some();
    let mut downloaded = false;

    let (remote_document, remote_etag) = match &remote_file {
        Some(file) => {
            let cached_etag = db::get_meta(connection, META_DRIVE_ETAG)?;
            match client.download(&file.id, cached_etag.as_deref()).await? {
                Download::NotModified => {
                    let cached = db::get_meta(connection, META_LAST_REMOTE)?.ok_or_else(|| {
                        CoreError::Invalid("Drive returned 304 without a cache".into())
                    })?;
                    (serde_json::from_str(&cached)?, cached_etag.clone())
                }
                Download::New { bytes, etag } => {
                    downloaded = true;
                    (decode_document(&bytes)?, etag)
                }
            }
        }
        None => (SyncDocument::default(), None),
    };

    let mut merged = merge_and_apply(connection, &remote_document, now_ms)?;
    let state = db::get_sync_state(connection)?;

    let mut uploaded = false;
    let mut drive_file_id = remote_file.as_ref().map(|file| file.id.clone());
    let mut etag = remote_etag;

    if state.dirty || !remote_found {
        prune_tombstones(&mut merged, now_ms, TOMBSTONE_MAX_AGE_MS);
        merged.exported_at_ms = now_ms;
        let bytes = encode_document(&merged)?;
        let file = match &remote_file {
            Some(file) => client.update(&file.id, &bytes).await?,
            None => client.create(&bytes).await?,
        };
        drive_file_id = Some(file.id);
        etag = file.etag.or(etag);
        uploaded = true;
    }

    if let Some(file_id) = drive_file_id.as_deref() {
        db::set_sync_state(connection, Some(file_id), now_ms, false)?;
    } else {
        db::set_sync_state(connection, None, now_ms, false)?;
    }
    if let Some(etag) = etag {
        db::set_meta(connection, META_DRIVE_ETAG, &etag)?;
    }
    // Cache the remote (or newly uploaded) shape for the next 304 response.
    db::set_meta(
        connection,
        META_LAST_REMOTE,
        &serde_json::to_string(if uploaded { &merged } else { &remote_document })?,
    )?;

    Ok(SyncOutcome {
        downloaded,
        uploaded,
        remote_found,
        last_synced_at_ms: now_ms,
    })
}

/// Helper used by the host to get a book's status maps without a full sync.
pub fn document_for_book(connection: &Connection, book_hash: &str) -> Result<SyncDocument> {
    let mut document = collect_document(connection, crate::now_ms())?;
    document.progress.retain(|hash, _| hash == book_hash);
    document
        .annotations
        .retain(|_, annotation| annotation.book_hash == book_hash);
    document
        .book_collections
        .retain(|_, record| record.book_hash == book_hash);
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        get_progress, insert_book, list_annotations, list_collections, NewBook, Progress,
    };

    fn add_book(connection: &Connection) {
        insert_book(
            connection,
            &NewBook {
                book_hash: "hash-a".into(),
                source_uri: "C:/a.epub".into(),
                file_size: 100,
                format: "epub".into(),
                title: "Alpha".into(),
                author: "Author".into(),
                language: "en".into(),
                cover_rel_path: None,
                added_at_ms: 1,
            },
        )
        .unwrap();
    }

    #[test]
    fn collects_local_progress_and_annotations() {
        let connection = db::open_in_memory().unwrap();
        add_book(&connection);
        db::save_progress(
            &connection,
            &Progress {
                book_hash: "hash-a".into(),
                cfi: "epubcfi(/6/2)".into(),
                fraction: 0.3,
                updated_at_ms: 5,
                device_id: "win-a".into(),
            },
        )
        .unwrap();
        let document = collect_document(&connection, 10).unwrap();
        assert_eq!(document.progress.len(), 1);
        assert_eq!(document.progress["hash-a"].fraction, 0.3);
    }

    #[test]
    fn applies_remote_rows_to_a_matching_book() {
        let mut connection = db::open_in_memory().unwrap();
        add_book(&connection);
        db::set_sync_state(&connection, None, 0, false).unwrap();
        let mut remote = SyncDocument::default();
        remote.progress.insert(
            "hash-a".into(),
            ProgressRecord {
                cfi: "epubcfi(/6/4)".into(),
                fraction: 0.7,
                updated_at_ms: 100,
                device_id: "and-b".into(),
            },
        );
        remote.annotations.insert(
            "ann-1".into(),
            AnnotationRecord {
                book_hash: "hash-a".into(),
                kind: "highlight".into(),
                cfi: "epubcfi(/6/4!/2)".into(),
                color: "yellow".into(),
                quote_text: "quoted".into(),
                note_text: String::new(),
                created_at_ms: 10,
                updated_at_ms: 100,
                deleted_at_ms: None,
            },
        );
        let merged = merge_and_apply(&mut connection, &remote, 200).unwrap();
        assert_eq!(merged.progress.len(), 1);
        let progress = get_progress(&connection, "hash-a").unwrap().unwrap();
        assert_eq!(progress.fraction, 0.7);
        assert_eq!(
            list_annotations(&connection, "hash-a", false)
                .unwrap()
                .len(),
            1
        );
        assert!(!db::get_sync_state(&connection).unwrap().dirty);
    }

    #[test]
    fn keeps_orphan_records_in_the_document_but_not_sqlite() {
        let mut connection = db::open_in_memory().unwrap();
        add_book(&connection);
        let mut remote = SyncDocument::default();
        remote.progress.insert(
            "missing-book".into(),
            ProgressRecord {
                cfi: "epubcfi(/6/2)".into(),
                fraction: 0.1,
                updated_at_ms: 100,
                device_id: "and-b".into(),
            },
        );
        let merged = merge_and_apply(&mut connection, &remote, 200).unwrap();
        assert!(merged.progress.contains_key("missing-book"));
        assert!(get_progress(&connection, "missing-book").unwrap().is_none());
    }

    #[test]
    fn propagates_a_remote_tombstone() {
        let mut connection = db::open_in_memory().unwrap();
        add_book(&connection);
        db::upsert_annotation(
            &connection,
            &crate::db::Annotation {
                id: "ann-1".into(),
                book_hash: "hash-a".into(),
                kind: "highlight".into(),
                cfi: "epubcfi(/6/2)".into(),
                color: "yellow".into(),
                quote_text: "quoted".into(),
                note_text: String::new(),
                created_at_ms: 10,
                updated_at_ms: 10,
                deleted_at_ms: None,
            },
        )
        .unwrap();
        let mut remote = SyncDocument::default();
        remote.annotations.insert(
            "ann-1".into(),
            AnnotationRecord {
                book_hash: "hash-a".into(),
                kind: "highlight".into(),
                cfi: "epubcfi(/6/2)".into(),
                color: "yellow".into(),
                quote_text: "quoted".into(),
                note_text: String::new(),
                created_at_ms: 10,
                updated_at_ms: 200,
                deleted_at_ms: Some(200),
            },
        );
        merge_and_apply(&mut connection, &remote, 300).unwrap();
        assert!(list_annotations(&connection, "hash-a", false)
            .unwrap()
            .is_empty());
        assert_eq!(
            list_annotations(&connection, "hash-a", true).unwrap()[0].deleted_at_ms,
            Some(200)
        );
    }

    #[test]
    fn merges_collections_from_remote() {
        let mut connection = db::open_in_memory().unwrap();
        let mut remote = SyncDocument::default();
        remote.collections.insert(
            "c1".into(),
            CollectionRecord {
                name: "Remote".into(),
                updated_at_ms: 10,
                deleted_at_ms: None,
            },
        );
        merge_and_apply(&mut connection, &remote, 20).unwrap();
        assert_eq!(list_collections(&connection).unwrap().len(), 1);
        assert_eq!(list_collections(&connection).unwrap()[0].name, "Remote");
    }
}
