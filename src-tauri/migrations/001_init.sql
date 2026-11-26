CREATE TABLE IF NOT EXISTS device_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS books (
    book_hash TEXT PRIMARY KEY,              -- 64-char hex BLAKE3(first 64KB + last 64KB + u64_le size)
    source_uri TEXT NOT NULL,                -- Windows PathBuf or Android content:// URI
    file_size INTEGER NOT NULL,
    format TEXT NOT NULL,                    -- 'epub' | 'mobi' | 'azw3' | 'fb2' | 'cbz' | 'pdf'
    title TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    language TEXT NOT NULL DEFAULT '',
    cover_rel_path TEXT,                     -- Relative path to 300px WebP in app cache dir
    added_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER                    -- Soft-delete tombstone for sync propagation
);

CREATE TABLE IF NOT EXISTS reading_progress (
    book_hash TEXT PRIMARY KEY REFERENCES books(book_hash) ON DELETE CASCADE,
    cfi TEXT NOT NULL,                       -- EPUB CFI or normalized page locator
    fraction REAL NOT NULL DEFAULT 0.0,      -- 0.0 to 1.0 progress ratio
    updated_at_ms INTEGER NOT NULL,
    device_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,                     -- UUIDv7
    book_hash TEXT NOT NULL REFERENCES books(book_hash) ON DELETE CASCADE,
    kind TEXT NOT NULL,                      -- 'highlight' | 'bookmark'
    cfi TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT 'yellow',
    quote_text TEXT NOT NULL DEFAULT '',
    note_text TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER                    -- Non-null indicates tombstone
);

CREATE INDEX IF NOT EXISTS idx_annotations_book ON annotations(book_hash) WHERE deleted_at_ms IS NULL;

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,                     -- UUIDv7
    name TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER
);

CREATE TABLE IF NOT EXISTS book_collections (
    book_hash TEXT NOT NULL REFERENCES books(book_hash) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER,
    PRIMARY KEY (book_hash, collection_id)
);

CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    drive_file_id TEXT,                      -- Cached Google Drive appDataFolder fileId
    last_synced_at_ms INTEGER NOT NULL DEFAULT 0,
    dirty INTEGER NOT NULL DEFAULT 0         -- 1 if local writes occurred since last sync
);

INSERT OR IGNORE INTO sync_state (id, last_synced_at_ms, dirty) VALUES (1, 0, 0);
