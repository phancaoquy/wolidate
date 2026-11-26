use crate::{new_device_id, CoreError, Result};
use rusqlite::{params, OptionalExtension, Row};

pub use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The v0.1 schema. Kept next to the core crate so `include_str!` works in
/// tests and release builds without a runtime file lookup.
pub const SCHEMA: &str = include_str!("../../../migrations/001_init.sql");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub book_hash: String,
    pub source_uri: String,
    pub file_size: i64,
    pub format: String,
    pub title: String,
    pub author: String,
    pub language: String,
    pub cover_rel_path: Option<String>,
    pub added_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
    pub progress_fraction: f64,
    pub progress_cfi: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBook {
    pub book_hash: String,
    pub source_uri: String,
    pub file_size: i64,
    pub format: String,
    pub title: String,
    pub author: String,
    pub language: String,
    pub cover_rel_path: Option<String>,
    pub added_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub book_hash: String,
    pub cfi: String,
    pub fraction: f64,
    pub updated_at_ms: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub book_hash: String,
    pub kind: String,
    pub cfi: String,
    pub color: String,
    pub quote_text: String,
    pub note_text: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookCollection {
    pub book_hash: String,
    pub collection_id: String,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub drive_file_id: Option<String>,
    pub last_synced_at_ms: i64,
    pub dirty: bool,
}

/// Open a file-backed database, creating parent directories if needed.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    init(&connection)?;
    Ok(connection)
}

pub fn open_in_memory() -> Result<Connection> {
    let connection = Connection::open_in_memory()?;
    init(&connection)?;
    Ok(connection)
}

/// Configure SQLite and run the v0.1 migration.
pub fn init(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > 1 {
        return Err(CoreError::Invalid(format!(
            "database schema {version} is newer than this build"
        )));
    }
    if version == 0 {
        connection.execute_batch(SCHEMA)?;
        connection.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

const BOOK_COLUMNS: &str = "
    b.book_hash, b.source_uri, b.file_size, b.format, b.title, b.author,
    b.language, b.cover_rel_path, b.added_at_ms, b.deleted_at_ms,
    COALESCE(p.fraction, 0.0), p.cfi";

fn map_book(row: &Row<'_>) -> rusqlite::Result<Book> {
    Ok(Book {
        book_hash: row.get(0)?,
        source_uri: row.get(1)?,
        file_size: row.get(2)?,
        format: row.get(3)?,
        title: row.get(4)?,
        author: row.get(5)?,
        language: row.get(6)?,
        cover_rel_path: row.get(7)?,
        added_at_ms: row.get(8)?,
        deleted_at_ms: row.get(9)?,
        progress_fraction: row.get(10)?,
        progress_cfi: row.get(11)?,
    })
}

pub fn insert_book(connection: &Connection, book: &NewBook) -> Result<()> {
    connection.execute(
        "INSERT INTO books (
            book_hash, source_uri, file_size, format, title, author, language,
            cover_rel_path, added_at_ms, deleted_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)
         ON CONFLICT(book_hash) DO UPDATE SET
            source_uri = excluded.source_uri,
            file_size = excluded.file_size,
            format = excluded.format,
            title = excluded.title,
            author = excluded.author,
            language = excluded.language,
            cover_rel_path = excluded.cover_rel_path,
            deleted_at_ms = NULL",
        params![
            book.book_hash,
            book.source_uri,
            book.file_size,
            book.format,
            book.title,
            book.author,
            book.language,
            book.cover_rel_path,
            book.added_at_ms,
        ],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn get_book(connection: &Connection, book_hash: &str) -> Result<Option<Book>> {
    let sql = format!(
        "SELECT {BOOK_COLUMNS}
         FROM books b
         LEFT JOIN reading_progress p ON p.book_hash = b.book_hash
         WHERE b.book_hash = ?1"
    );
    connection
        .query_row(&sql, params![book_hash], map_book)
        .optional()
        .map_err(Into::into)
}

pub fn list_books(
    connection: &Connection,
    search: Option<&str>,
    collection_id: Option<&str>,
) -> Result<Vec<Book>> {
    let sql = format!(
        "SELECT {BOOK_COLUMNS}
         FROM books b
         LEFT JOIN reading_progress p ON p.book_hash = b.book_hash
         WHERE b.deleted_at_ms IS NULL
           AND (?1 = '' OR b.title LIKE ?1 ESCAPE '\\' OR b.author LIKE ?1 ESCAPE '\\')
           AND (?2 IS NULL OR EXISTS (
                SELECT 1 FROM book_collections bc
                JOIN collections c ON c.id = bc.collection_id
                WHERE bc.book_hash = b.book_hash
                  AND bc.collection_id = ?2
                  AND bc.deleted_at_ms IS NULL
                  AND c.deleted_at_ms IS NULL
           ))
         ORDER BY b.added_at_ms DESC"
    );
    let pattern = like_pattern(search.unwrap_or_default());
    let mut stmt = connection.prepare(&sql)?;
    let rows = stmt.query_map(params![pattern, collection_id], map_book)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn soft_delete_book(
    connection: &Connection,
    book_hash: &str,
    deleted_at_ms: i64,
) -> Result<()> {
    connection.execute(
        "UPDATE books SET deleted_at_ms = ?2 WHERE book_hash = ?1",
        params![book_hash, deleted_at_ms],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn save_progress(connection: &Connection, progress: &Progress) -> Result<()> {
    connection.execute(
        "INSERT INTO reading_progress (book_hash, cfi, fraction, updated_at_ms, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(book_hash) DO UPDATE SET
            cfi = excluded.cfi,
            fraction = excluded.fraction,
            updated_at_ms = excluded.updated_at_ms,
            device_id = excluded.device_id",
        params![
            progress.book_hash,
            progress.cfi,
            progress.fraction,
            progress.updated_at_ms,
            progress.device_id,
        ],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn get_progress(connection: &Connection, book_hash: &str) -> Result<Option<Progress>> {
    connection
        .query_row(
            "SELECT book_hash, cfi, fraction, updated_at_ms, device_id
             FROM reading_progress WHERE book_hash = ?1",
            params![book_hash],
            |row| {
                Ok(Progress {
                    book_hash: row.get(0)?,
                    cfi: row.get(1)?,
                    fraction: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                    device_id: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub fn upsert_annotation(connection: &Connection, annotation: &Annotation) -> Result<()> {
    connection.execute(
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
            annotation.id,
            annotation.book_hash,
            annotation.kind,
            annotation.cfi,
            annotation.color,
            annotation.quote_text,
            annotation.note_text,
            annotation.created_at_ms,
            annotation.updated_at_ms,
            annotation.deleted_at_ms,
        ],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn list_annotations(
    connection: &Connection,
    book_hash: &str,
    include_deleted: bool,
) -> Result<Vec<Annotation>> {
    let mut stmt = connection.prepare(
        "SELECT id, book_hash, kind, cfi, color, quote_text, note_text,
                created_at_ms, updated_at_ms, deleted_at_ms
         FROM annotations
         WHERE book_hash = ?1 AND (?2 = 1 OR deleted_at_ms IS NULL)
         ORDER BY created_at_ms ASC",
    )?;
    let rows = stmt.query_map(params![book_hash, include_deleted], |row| {
        Ok(Annotation {
            id: row.get(0)?,
            book_hash: row.get(1)?,
            kind: row.get(2)?,
            cfi: row.get(3)?,
            color: row.get(4)?,
            quote_text: row.get(5)?,
            note_text: row.get(6)?,
            created_at_ms: row.get(7)?,
            updated_at_ms: row.get(8)?,
            deleted_at_ms: row.get(9)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_annotation(connection: &Connection, id: &str) -> Result<Option<Annotation>> {
    connection
        .query_row(
            "SELECT id, book_hash, kind, cfi, color, quote_text, note_text,
                    created_at_ms, updated_at_ms, deleted_at_ms
             FROM annotations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Annotation {
                    id: row.get(0)?,
                    book_hash: row.get(1)?,
                    kind: row.get(2)?,
                    cfi: row.get(3)?,
                    color: row.get(4)?,
                    quote_text: row.get(5)?,
                    note_text: row.get(6)?,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                    deleted_at_ms: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub fn soft_delete_annotation(connection: &Connection, id: &str, updated_at_ms: i64) -> Result<()> {
    connection.execute(
        "UPDATE annotations SET deleted_at_ms = ?2, updated_at_ms = ?2 WHERE id = ?1",
        params![id, updated_at_ms],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn list_collections(connection: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = connection.prepare(
        "SELECT id, name, updated_at_ms, deleted_at_ms
         FROM collections WHERE deleted_at_ms IS NULL ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Collection {
            id: row.get(0)?,
            name: row.get(1)?,
            updated_at_ms: row.get(2)?,
            deleted_at_ms: row.get(3)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_collection(connection: &Connection, id: &str) -> Result<Option<Collection>> {
    connection
        .query_row(
            "SELECT id, name, updated_at_ms, deleted_at_ms FROM collections WHERE id = ?1",
            params![id],
            |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    updated_at_ms: row.get(2)?,
                    deleted_at_ms: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub fn upsert_collection(connection: &Connection, collection: &Collection) -> Result<()> {
    connection.execute(
        "INSERT INTO collections (id, name, updated_at_ms, deleted_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            updated_at_ms = excluded.updated_at_ms,
            deleted_at_ms = excluded.deleted_at_ms",
        params![
            collection.id,
            collection.name,
            collection.updated_at_ms,
            collection.deleted_at_ms,
        ],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn create_collection(connection: &Connection, name: &str, now: i64) -> Result<Collection> {
    let collection = Collection {
        id: uuid::Uuid::now_v7().to_string(),
        name: name.trim().to_owned(),
        updated_at_ms: now,
        deleted_at_ms: None,
    };
    upsert_collection(connection, &collection)?;
    Ok(collection)
}

pub fn soft_delete_collection(
    connection: &Connection,
    collection_id: &str,
    updated_at_ms: i64,
) -> Result<()> {
    connection.execute(
        "UPDATE collections SET deleted_at_ms = ?2, updated_at_ms = ?2 WHERE id = ?1",
        params![collection_id, updated_at_ms],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn set_book_collection(
    connection: &Connection,
    book_hash: &str,
    collection_id: &str,
    added: bool,
    updated_at_ms: i64,
) -> Result<()> {
    let deleted_at_ms = if added { None } else { Some(updated_at_ms) };
    connection.execute(
        "INSERT INTO book_collections (book_hash, collection_id, updated_at_ms, deleted_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(book_hash, collection_id) DO UPDATE SET
            updated_at_ms = excluded.updated_at_ms,
            deleted_at_ms = excluded.deleted_at_ms",
        params![book_hash, collection_id, updated_at_ms, deleted_at_ms],
    )?;
    mark_dirty(connection)?;
    Ok(())
}

pub fn list_book_collections(connection: &Connection) -> Result<Vec<BookCollection>> {
    let mut stmt = connection.prepare(
        "SELECT book_hash, collection_id, updated_at_ms, deleted_at_ms
         FROM book_collections",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(BookCollection {
            book_hash: row.get(0)?,
            collection_id: row.get(1)?,
            updated_at_ms: row.get(2)?,
            deleted_at_ms: row.get(3)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_sync_state(connection: &Connection) -> Result<SyncState> {
    connection
        .query_row(
            "SELECT drive_file_id, last_synced_at_ms, dirty FROM sync_state WHERE id = 1",
            [],
            |row| {
                Ok(SyncState {
                    drive_file_id: row.get(0)?,
                    last_synced_at_ms: row.get(1)?,
                    dirty: row.get::<_, i64>(2)? != 0,
                })
            },
        )
        .map_err(Into::into)
}

pub fn set_sync_state(
    connection: &Connection,
    drive_file_id: Option<&str>,
    last_synced_at_ms: i64,
    dirty: bool,
) -> Result<()> {
    connection.execute(
        "UPDATE sync_state
         SET drive_file_id = ?1, last_synced_at_ms = ?2, dirty = ?3
         WHERE id = 1",
        params![drive_file_id, last_synced_at_ms, i64::from(dirty)],
    )?;
    Ok(())
}

pub fn mark_dirty(connection: &Connection) -> Result<()> {
    connection.execute("UPDATE sync_state SET dirty = 1 WHERE id = 1", [])?;
    Ok(())
}

pub fn get_meta(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT value FROM device_meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

pub fn set_meta(connection: &Connection, key: &str, value: &str) -> Result<()> {
    connection.execute(
        "INSERT INTO device_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Return the stable device id, creating one on first launch.
pub fn ensure_device_id(connection: &Connection, prefix: &str) -> Result<String> {
    if let Some(id) = get_meta(connection, "device_id")? {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = new_device_id(prefix);
    set_meta(connection, "device_id", &id)?;
    Ok(id)
}

/// Count non-deleted books. Used by tests and the empty state.
pub fn book_count(connection: &Connection) -> Result<i64> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM books WHERE deleted_at_ms IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_book(hash: &str, title: &str) -> NewBook {
        NewBook {
            book_hash: hash.into(),
            source_uri: format!("C:/books/{hash}.epub"),
            file_size: 1234,
            format: "epub".into(),
            title: title.into(),
            author: "Author".into(),
            language: "en".into(),
            cover_rel_path: None,
            added_at_ms: 100,
        }
    }

    #[test]
    fn initializes_and_stores_books() {
        let connection = open_in_memory().unwrap();
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        insert_book(&connection, &sample_book("hash-b", "Beta")).unwrap();
        assert_eq!(book_count(&connection).unwrap(), 2);

        let books = list_books(&connection, Some("alp"), None).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Alpha");
        assert_eq!(books[0].progress_fraction, 0.0);
        assert!(get_sync_state(&connection).unwrap().dirty);
    }

    #[test]
    fn reimport_restores_a_soft_deleted_book() {
        let connection = open_in_memory().unwrap();
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        soft_delete_book(&connection, "hash-a", 200).unwrap();
        assert_eq!(book_count(&connection).unwrap(), 0);
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        assert_eq!(book_count(&connection).unwrap(), 1);
    }

    #[test]
    fn progress_is_upserted_and_joined_into_the_book() {
        let connection = open_in_memory().unwrap();
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        save_progress(
            &connection,
            &Progress {
                book_hash: "hash-a".into(),
                cfi: "epubcfi(/6/2)".into(),
                fraction: 0.25,
                updated_at_ms: 300,
                device_id: "win-test".into(),
            },
        )
        .unwrap();
        save_progress(
            &connection,
            &Progress {
                book_hash: "hash-a".into(),
                cfi: "epubcfi(/6/4)".into(),
                fraction: 0.5,
                updated_at_ms: 400,
                device_id: "win-test".into(),
            },
        )
        .unwrap();
        let book = get_book(&connection, "hash-a").unwrap().unwrap();
        assert_eq!(book.progress_fraction, 0.5);
        assert_eq!(book.progress_cfi.as_deref(), Some("epubcfi(/6/4)"));
    }

    #[test]
    fn annotations_are_hidden_after_soft_delete() {
        let connection = open_in_memory().unwrap();
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        let annotation = Annotation {
            id: "ann-1".into(),
            book_hash: "hash-a".into(),
            kind: "highlight".into(),
            cfi: "epubcfi(/6/2!/4/2)".into(),
            color: "yellow".into(),
            quote_text: "quote".into(),
            note_text: String::new(),
            created_at_ms: 10,
            updated_at_ms: 10,
            deleted_at_ms: None,
        };
        upsert_annotation(&connection, &annotation).unwrap();
        assert_eq!(
            list_annotations(&connection, "hash-a", false)
                .unwrap()
                .len(),
            1
        );
        soft_delete_annotation(&connection, "ann-1", 20).unwrap();
        assert_eq!(
            list_annotations(&connection, "hash-a", false)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            list_annotations(&connection, "hash-a", true).unwrap().len(),
            1
        );
    }

    #[test]
    fn collections_filter_books() {
        let connection = open_in_memory().unwrap();
        insert_book(&connection, &sample_book("hash-a", "Alpha")).unwrap();
        insert_book(&connection, &sample_book("hash-b", "Beta")).unwrap();
        let collection = create_collection(&connection, "Systems", 10).unwrap();
        set_book_collection(&connection, "hash-a", &collection.id, true, 11).unwrap();
        let filtered = list_books(&connection, None, Some(&collection.id)).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].book_hash, "hash-a");
        set_book_collection(&connection, "hash-a", &collection.id, false, 12).unwrap();
        assert!(list_books(&connection, None, Some(&collection.id))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn device_id_is_stable() {
        let connection = open_in_memory().unwrap();
        let first = ensure_device_id(&connection, "win").unwrap();
        let second = ensure_device_id(&connection, "win").unwrap();
        assert_eq!(first, second);
        assert!(first.starts_with("win-"));
    }

    #[test]
    fn sync_state_round_trips() {
        let connection = open_in_memory().unwrap();
        set_sync_state(&connection, Some("drive-1"), 555, false).unwrap();
        let state = get_sync_state(&connection).unwrap();
        assert_eq!(state.drive_file_id.as_deref(), Some("drive-1"));
        assert_eq!(state.last_synced_at_ms, 555);
        assert!(!state.dirty);
        mark_dirty(&connection).unwrap();
        assert!(get_sync_state(&connection).unwrap().dirty);
    }
}
