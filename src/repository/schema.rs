use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::error::{NtError, Result};

pub(super) const APPLICATION_ID: i64 = 0x4e54_4e54;
pub(super) const SCHEMA_VERSION: i64 = 1;

const SCHEMA_STEPS: &[&str] = &[
    "CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 1)
     ) WITHOUT ROWID",
    "CREATE TABLE notes (
         pk INTEGER PRIMARY KEY,
         id TEXT NOT NULL UNIQUE,
         collection TEXT NOT NULL,
         body TEXT NOT NULL,
         title TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         body_version INTEGER NOT NULL DEFAULT 1,
         CHECK(length(id) = 36
               AND substr(id, 9, 1) = '-'
               AND substr(id, 14, 1) = '-'
               AND substr(id, 15, 1) = '7'
               AND substr(id, 19, 1) = '-'
               AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
               AND substr(id, 24, 1) = '-'
               AND length(replace(id, '-', '')) = 32
               AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
         CHECK(length(collection) > 0
               AND collection NOT GLOB '*[^a-z0-9_/-]*'
               AND substr(collection, 1, 1) <> '/'
               AND substr(collection, -1, 1) <> '/'
               AND instr(collection, '//') = 0),
         CHECK(length(body) > 0),
         CHECK(length(title) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(updated GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(body_version > 0)
     )",
    "CREATE TABLE note_tags (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         tag TEXT NOT NULL,
         PRIMARY KEY(note_pk, tag),
         CHECK(length(tag) > 0 AND tag NOT GLOB '*[^a-z0-9_-]*')
     )",
    "CREATE TABLE note_links (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         target_note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         PRIMARY KEY(note_pk, target_note_pk),
         CHECK(note_pk <> target_note_pk)
     )",
    "CREATE VIRTUAL TABLE note_fts USING fts5(
         title,
         body,
         content = 'notes',
         content_rowid = 'pk',
         tokenize = 'unicode61 remove_diacritics 2'
     )",
    "CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END",
    "CREATE TRIGGER notes_fts_update AFTER UPDATE OF title, body ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END",
    "CREATE TRIGGER notes_fts_delete BEFORE DELETE ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
     END",
    "CREATE INDEX notes_created_idx ON notes(created DESC, id DESC)",
    "CREATE INDEX notes_updated_idx ON notes(updated DESC, id DESC)",
    "CREATE INDEX notes_collection_updated_idx
         ON notes(collection, updated DESC, id DESC)",
    "CREATE INDEX note_tags_tag_note_idx ON note_tags(tag, note_pk)",
    "CREATE INDEX note_links_target_idx ON note_links(target_note_pk)",
    "INSERT INTO schema_version(singleton, version) VALUES (1, 1)",
];

const REQUIRED_OBJECTS: &[(&str, &str)] = &[
    ("table", "schema_version"),
    ("table", "notes"),
    ("table", "note_tags"),
    ("table", "note_links"),
    ("table", "note_fts"),
    ("trigger", "notes_fts_insert"),
    ("trigger", "notes_fts_update"),
    ("trigger", "notes_fts_delete"),
    ("index", "notes_created_idx"),
    ("index", "notes_updated_idx"),
    ("index", "notes_collection_updated_idx"),
    ("index", "note_tags_tag_note_idx"),
    ("index", "note_links_target_idx"),
];

const FTS_SHADOW_OBJECTS: &[&str] = &[
    "note_fts_data",
    "note_fts_idx",
    "note_fts_docsize",
    "note_fts_config",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Identity {
    Empty,
    Nt,
}

pub(super) fn inspect(connection: &Connection) -> Result<Identity> {
    let application_id: i64 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let object_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;

    if application_id == 0 {
        return if object_count == 0 {
            Ok(Identity::Empty)
        } else {
            Err(NtError::NotNtDatabase)
        };
    }
    if application_id != APPLICATION_ID {
        return Err(NtError::NotNtDatabase);
    }

    validate_version(connection)?;
    validate_schema(connection)?;
    Ok(Identity::Nt)
}

pub(super) fn initialize(connection: &mut Connection) -> Result<bool> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match inspect(&transaction)? {
        Identity::Nt => {
            transaction.commit()?;
            Ok(false)
        }
        Identity::Empty => {
            initialize_transaction(&transaction, |_| Ok(()))?;
            transaction.commit()?;
            Ok(true)
        }
    }
}

#[cfg(test)]
fn initialize_with(
    connection: &mut Connection,
    mut after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    initialize_transaction(&transaction, &mut after_step)?;
    transaction.commit()?;
    Ok(())
}

fn initialize_transaction(
    transaction: &rusqlite::Transaction<'_>,
    mut after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    for (step, sql) in SCHEMA_STEPS.iter().enumerate() {
        transaction.execute_batch(sql)?;
        after_step(step)?;
    }
    transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
    after_step(SCHEMA_STEPS.len())?;
    validate_version(transaction)?;
    validate_schema(transaction)?;
    Ok(())
}

pub(super) fn configure(connection: &Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(())
}

pub(super) fn configure_wal(connection: &Connection) -> Result<()> {
    configure(connection)?;
    connection.execute_batch("PRAGMA journal_mode = WAL")?;
    Ok(())
}

fn validate_version(connection: &Connection) -> Result<()> {
    let has_table = connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_table {
        return Err(NtError::NotNtDatabase);
    }

    let mut statement = connection.prepare("SELECT version FROM schema_version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match versions.as_slice() {
        [SCHEMA_VERSION] => Ok(()),
        [version] => Err(NtError::UnsupportedSchema(*version)),
        _ => Err(NtError::NotNtDatabase),
    }
}

fn validate_schema(connection: &Connection) -> Result<()> {
    for &(object_type, name) in REQUIRED_OBJECTS {
        let exists = connection
            .query_row(
                "SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                (object_type, name),
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(NtError::NotNtDatabase);
        }
    }

    let singleton: Option<i64> = connection
        .query_row(
            "SELECT singleton FROM schema_version WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if singleton != Some(1) {
        return Err(NtError::NotNtDatabase);
    }
    let object_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if object_count != (REQUIRED_OBJECTS.len() + FTS_SHADOW_OBJECTS.len()) as i64 {
        return Err(NtError::NotNtDatabase);
    }
    for name in FTS_SHADOW_OBJECTS {
        let exists = connection
            .query_row(
                "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(NtError::NotNtDatabase);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialized() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        assert!(initialize(&mut connection).unwrap());
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .unwrap();
        connection
    }

    #[test]
    fn initializes_version_one_with_nt_identity() {
        let mut connection = initialized();
        assert_eq!(inspect(&connection).unwrap(), Identity::Nt);
        assert!(!initialize(&mut connection).unwrap());
        let application_id: i64 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .unwrap();
        assert_eq!(application_id, APPLICATION_ID);
        let version: i64 = connection
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn every_failed_initialization_step_rolls_back_identity_and_schema() {
        for failed_step in 0..=SCHEMA_STEPS.len() {
            let mut connection = Connection::open_in_memory().unwrap();
            let result = initialize_with(&mut connection, |step| {
                if step == failed_step {
                    return Err(NtError::Message("injected initialization failure".into()));
                }
                Ok(())
            });
            assert!(result.is_err(), "step {failed_step} unexpectedly succeeded");
            assert_eq!(inspect(&connection).unwrap(), Identity::Empty);
        }
    }

    #[test]
    fn schema_enforces_relationship_constraints_and_cascades() {
        let connection = initialized();
        connection
            .execute_batch(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                     VALUES ('018fbe0a-6c00-7000-8000-000000000001',
                             'inbox', '# A', 'A', '2026-05-28T14:30:12Z',
                             '2026-05-28T14:30:12Z');
                 INSERT INTO notes(id, collection, body, title, created, updated)
                     VALUES ('018fbe0a-6c00-7000-8000-000000000002',
                             'inbox', '# B', 'B', '2026-05-28T14:30:12Z',
                             '2026-05-28T14:30:12Z');
                 INSERT INTO note_tags(note_pk, tag) VALUES (1, 'rust');
                 INSERT INTO note_links(note_pk, target_note_pk) VALUES (1, 2);",
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO note_links(note_pk, target_note_pk) VALUES (1, 1)",
                    [],
                )
                .is_err()
        );
        connection
            .execute("DELETE FROM notes WHERE pk = 1", [])
            .unwrap();
        let tags: i64 = connection
            .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
            .unwrap();
        let links: i64 = connection
            .query_row("SELECT COUNT(*) FROM note_links", [], |row| row.get(0))
            .unwrap();
        assert_eq!((tags, links), (0, 0));
    }

    #[test]
    fn schema_enforces_cheap_canonical_value_shapes() {
        let connection = initialized();
        let insert_note = |id: &str, collection: &str, created: &str, updated: &str| {
            connection.execute(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                 VALUES (?1, ?2, '# Valid', 'Valid', ?3, ?4)",
                (id, collection, created, updated),
            )
        };
        let valid_id = "018fbe0a-6c00-7000-8000-000000000001";
        let valid_timestamp = "2026-05-28T14:30:12Z";

        for id in [
            "018fbe0a6c0070008000000000000001",
            "018FBE0A-6C00-7000-8000-000000000001",
            "018fbe0a-6c00-4000-8000-000000000001",
            "018fbe0a-6c00-7000-c000-000000000001",
        ] {
            assert!(
                insert_note(id, "inbox", valid_timestamp, valid_timestamp).is_err(),
                "accepted invalid id {id:?}"
            );
        }
        for collection in ["", "/inbox", "inbox/", "work//nt", "Work/nt", "work/a.b"] {
            assert!(
                insert_note(valid_id, collection, valid_timestamp, valid_timestamp).is_err(),
                "accepted invalid collection {collection:?}"
            );
        }
        for timestamp in ["2026/05/28T14:30:12Z", "2026-05-28 14:30:12Z"] {
            assert!(
                insert_note(valid_id, "inbox", timestamp, valid_timestamp).is_err(),
                "accepted invalid timestamp {timestamp:?}"
            );
            assert!(
                insert_note(valid_id, "inbox", valid_timestamp, timestamp).is_err(),
                "accepted invalid timestamp {timestamp:?}"
            );
        }

        insert_note(valid_id, "work/nt", valid_timestamp, valid_timestamp).unwrap();
        for tag in ["", "Rust", "rust/sqlite", "rust.sqlite"] {
            assert!(
                connection
                    .execute("INSERT INTO note_tags(note_pk, tag) VALUES (1, ?1)", [tag])
                    .is_err(),
                "accepted invalid tag {tag:?}"
            );
        }
        connection
            .execute(
                "INSERT INTO note_tags(note_pk, tag) VALUES (1, 'rust_2026')",
                [],
            )
            .unwrap();
    }

    #[test]
    fn fts_triggers_follow_content_changes_only() {
        let connection = initialized();
        connection
            .execute_batch(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                     VALUES ('018fbe0a-6c00-7000-8000-000000000001',
                             'inbox', '# Storage', 'Storage',
                             '2026-05-28T14:30:12Z', '2026-05-28T14:30:12Z');",
            )
            .unwrap();
        let matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM note_fts WHERE note_fts MATCH 'storage'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(matches, 1);

        connection
            .execute("UPDATE notes SET collection = 'work' WHERE pk = 1", [])
            .unwrap();
        connection
            .execute(
                "UPDATE notes SET body = '# Ownership', title = 'Ownership' WHERE pk = 1",
                [],
            )
            .unwrap();
        let matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM note_fts WHERE note_fts MATCH 'ownership'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(matches, 1);
        let old_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM note_fts WHERE note_fts MATCH 'storage'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_matches, 0);
        connection
            .execute("DELETE FROM notes WHERE pk = 1", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO note_fts(note_fts) VALUES ('integrity-check')",
                [],
            )
            .unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM note_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn identity_rejects_additional_schema_objects() {
        let connection = initialized();
        connection
            .execute_batch("CREATE TABLE unrelated(value TEXT)")
            .unwrap();
        assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
    }
}
