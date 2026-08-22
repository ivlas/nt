use rusqlite::Connection;

use super::*;
use crate::error::{NtError, Result};
use crate::note::{CollectionPath, NewNote, NoteQuery, Repository};

fn open_repository(path: &std::path::Path) -> Result<Repository> {
    open_read_write(path).map(Repository::from_connection)
}

fn open_read_only_repository(path: &std::path::Path) -> Result<Repository> {
    open_read_only(path).map(Repository::from_connection)
}

fn initialized() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    assert!(initialize(&mut connection).unwrap());
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .unwrap();
    connection
}

#[test]
fn initializes_current_schema_with_nt_identity() {
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
    for failed_step in 0..=MANIFEST.step_count() {
        let mut connection = Connection::open_in_memory().unwrap();
        let result = initialize_with(&mut connection, |step| {
            if step == failed_step {
                return Err(NtError::Io(std::io::Error::other(
                    "injected initialization failure",
                )));
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
fn validation_tolerates_additional_schema_objects() {
    let connection = initialized();
    connection
        .execute_batch(
            "CREATE TABLE unrelated(value TEXT);
             CREATE VIEW note_titles AS SELECT id, title FROM notes;
             CREATE INDEX unrelated_value_idx ON unrelated(value)",
        )
        .unwrap();
    assert_eq!(inspect(&connection).unwrap(), Identity::Nt);
}

#[test]
fn validation_rejects_unknown_triggers() {
    let connection = initialized();
    connection
        .execute_batch(
            "CREATE TRIGGER rewrite_note_title AFTER INSERT ON notes BEGIN
                 UPDATE notes SET title = 'changed' WHERE pk = new.pk;
             END",
        )
        .unwrap();

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn validation_rejects_changed_required_index_definition() {
    let connection = initialized();
    connection
        .execute_batch(
            "DROP INDEX notes_created_idx;
             CREATE INDEX notes_created_idx ON notes(id)",
        )
        .unwrap();

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn validation_rejects_changed_required_trigger_definition() {
    let connection = initialized();
    connection
        .execute_batch(
            "DROP TRIGGER notes_fts_insert;
             CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
                 SELECT 1;
             END",
        )
        .unwrap();

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn validation_rejects_changed_required_table_definition() {
    let connection = initialized();
    connection
        .execute_batch("ALTER TABLE notes ADD COLUMN unexpected TEXT")
        .unwrap();

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn validation_rejects_changed_fts_definition() {
    let connection = initialized();
    connection
        .execute_batch(
            "DROP TRIGGER notes_fts_insert;
             DROP TRIGGER notes_fts_update;
             DROP TRIGGER notes_fts_delete;
             DROP TABLE note_fts;
             CREATE VIRTUAL TABLE note_fts USING fts5(
                 title, body, content = 'notes', content_rowid = 'pk', tokenize = 'ascii'
             )",
        )
        .unwrap();
    for sql in &MANIFEST.steps()[5..8] {
        connection.execute_batch(sql).unwrap();
    }

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn malformed_schema_version_shape_is_a_schema_mismatch() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(&format!(
            "PRAGMA application_id = {APPLICATION_ID};
             CREATE TABLE schema_version(singleton INTEGER)"
        ))
        .unwrap();

    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));

    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(&format!(
            "PRAGMA application_id = {APPLICATION_ID};
             CREATE TABLE schema_version(singleton INTEGER, version TEXT);
             INSERT INTO schema_version VALUES (1, 'invalid')"
        ))
        .unwrap();
    assert!(matches!(inspect(&connection), Err(NtError::NotNtDatabase)));
}

#[test]
fn read_only_connections_read_notes_and_reject_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_at(&path).unwrap();
    let id = {
        let mut writer = open_repository(&path).unwrap();
        writer
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Read only")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap()
    };

    let reader = open_read_only_repository(&path).unwrap();
    let foreign_keys: i64 = reader
        .connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .unwrap();
    let journal: String = reader
        .connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!((foreign_keys, journal.as_str()), (1, "wal"));
    drop(reader);

    let mut reader = open_read_only_repository(&path).unwrap();
    assert_eq!(reader.get_note(&id).unwrap().body(), "# Read only");
    assert_eq!(reader.list_tags().unwrap(), vec!["rust".parse().unwrap()]);
    let mut visited = 0;
    reader
        .visit_note_summaries(&NoteQuery::default(), |_| {
            visited += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(visited, 1);

    assert!(
        reader
            .create_note(NewNote::new(CollectionPath::inbox(), "# Denied").unwrap())
            .is_err()
    );
}

#[test]
fn writer_contention_returns_the_retryable_busy_error() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_at(&path).unwrap();
    let mut first = open_repository(&path).unwrap();
    let mut second = open_repository(&path).unwrap();
    second
        .connection
        .busy_timeout(std::time::Duration::from_millis(1))
        .unwrap();
    let transaction = first
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let result = second.create_note(NewNote::new(CollectionPath::inbox(), "# Contended").unwrap());
    assert!(matches!(result, Err(NtError::DatabaseBusy)));
    transaction.rollback().unwrap();
}
