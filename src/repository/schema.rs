use rusqlite::{Connection, OptionalExtension};

use crate::error::{NtError, Result};

const SCHEMA_VERSION: i64 = 2;

pub(super) fn configure_and_initialize(connection: &Connection) -> Result<()> {
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    let has_schema_version = connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    let version = if has_schema_version {
        let version = connection
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(version) = version
            && version != SCHEMA_VERSION
        {
            return Err(NtError::Message(format!(
                "unsupported database schema version {version}"
            )));
        }
        version
    } else {
        None
    };

    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;",
    )?;

    if matches!(version, Some(SCHEMA_VERSION)) {
        return Ok(());
    }

    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
             version INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS vaults (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL UNIQUE,
             created TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS collections (
             id TEXT PRIMARY KEY,
             vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
             name TEXT NOT NULL,
             created TEXT NOT NULL,
             UNIQUE(vault_id, name)
         );
         CREATE TABLE IF NOT EXISTS notes (
             id TEXT PRIMARY KEY,
             home_collection_id TEXT NOT NULL,
             body TEXT NOT NULL,
             created TEXT NOT NULL,
             updated TEXT NOT NULL,
             title TEXT NOT NULL,
             kind TEXT NOT NULL CHECK(kind IN ('note', 'todo')),
             status TEXT,
             priority TEXT,
             scheduled TEXT,
             due TEXT,
             closed TEXT,
             UNIQUE(id, home_collection_id),
             FOREIGN KEY(id, home_collection_id)
                 REFERENCES note_collections(note_id, collection_id)
                 DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TABLE IF NOT EXISTS note_collections (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE RESTRICT,
             PRIMARY KEY(note_id, collection_id)
         );
         CREATE TABLE IF NOT EXISTS note_tags (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             tag TEXT NOT NULL,
             PRIMARY KEY(note_id, tag)
         );
         CREATE TABLE IF NOT EXISTS note_sources (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             source TEXT NOT NULL,
             PRIMARY KEY(note_id, source)
         );
         CREATE TABLE IF NOT EXISTS note_links (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             PRIMARY KEY(note_id, target_id)
         );
         CREATE TABLE IF NOT EXISTS note_search_rows (
             search_id INTEGER PRIMARY KEY,
             note_id TEXT NOT NULL UNIQUE REFERENCES notes(id) ON DELETE CASCADE
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS note_fts USING fts5(
             title,
             body,
             content = '',
             tokenize = 'unicode61 remove_diacritics 2'
         );
         CREATE TRIGGER IF NOT EXISTS notes_search_insert
         AFTER INSERT ON notes BEGIN
             INSERT INTO note_search_rows (note_id) VALUES (new.id);
             INSERT INTO note_fts (rowid, title, body)
             SELECT search_id, new.title, new.body
             FROM note_search_rows WHERE note_id = new.id;
         END;
         CREATE TRIGGER IF NOT EXISTS notes_search_update
         AFTER UPDATE OF title, body ON notes BEGIN
             INSERT INTO note_fts (note_fts, rowid, title, body)
             SELECT 'delete', search_id, old.title, old.body
             FROM note_search_rows WHERE note_id = old.id;
             INSERT INTO note_fts (rowid, title, body)
             SELECT search_id, new.title, new.body
             FROM note_search_rows WHERE note_id = new.id;
         END;
         CREATE TRIGGER IF NOT EXISTS notes_search_delete
         BEFORE DELETE ON notes BEGIN
             INSERT INTO note_fts (note_fts, rowid, title, body)
             SELECT 'delete', search_id, old.title, old.body
             FROM note_search_rows WHERE note_id = old.id;
         END;
         DROP INDEX IF EXISTS note_tags_tag;
         CREATE INDEX IF NOT EXISTS notes_created ON notes(created DESC, id DESC);
         CREATE INDEX IF NOT EXISTS notes_open_todos_created
             ON notes(created DESC, id DESC)
             WHERE kind = 'todo' AND status = 'open';
         CREATE INDEX IF NOT EXISTS notes_status_created
             ON notes(LOWER(status), created DESC, id DESC)
             WHERE status IS NOT NULL;
         CREATE INDEX IF NOT EXISTS notes_created_day
             ON notes(substr(created, 1, 10), created DESC, id DESC);
         CREATE INDEX IF NOT EXISTS notes_id_nocase ON notes(id COLLATE NOCASE);
         CREATE INDEX IF NOT EXISTS note_collections_collection ON note_collections(collection_id);
         CREATE INDEX IF NOT EXISTS note_tags_lower_tag_note
             ON note_tags(LOWER(tag), note_id);
         CREATE INDEX IF NOT EXISTS note_links_target ON note_links(target_id);",
    )?;
    connection.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        [SCHEMA_VERSION],
    )?;
    Ok(())
}

pub(super) fn is_nt_database(connection: &Connection) -> Result<bool> {
    let has_schema_version = connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_schema_version {
        return Ok(false);
    }

    let version: Option<i64> = connection
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    match version {
        Some(SCHEMA_VERSION) => {}
        Some(version) => {
            return Err(NtError::Message(format!(
                "unsupported database schema version {version}"
            )));
        }
        None => return Ok(false),
    }

    let table_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema
         WHERE type = 'table' AND name IN (
             'schema_version', 'vaults', 'collections', 'notes',
             'note_collections', 'note_tags', 'note_sources', 'note_links'
         )",
        [],
        |row| row.get(0),
    )?;
    Ok(table_count == 8)
}
