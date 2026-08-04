use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::error::{NtError, Result};

const SCHEMA_VERSION: i64 = 3;

const SCHEMA_STEPS: &[&str] = &[
    "CREATE TABLE schema_version (
         singleton INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL
     ) WITHOUT ROWID",
    "CREATE TABLE vaults (
         id TEXT PRIMARY KEY,
         name TEXT NOT NULL UNIQUE,
         created TEXT NOT NULL
     )",
    "CREATE TABLE collections (
         id TEXT PRIMARY KEY,
         vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
         name TEXT NOT NULL,
         created TEXT NOT NULL,
         UNIQUE(vault_id, name)
     )",
    "CREATE TABLE notes (
         id TEXT PRIMARY KEY,
         home_collection_id TEXT NOT NULL,
         body TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         title TEXT NOT NULL,
         kind TEXT NOT NULL CHECK(kind IN ('note', 'todo')),
         status TEXT CHECK(status IS NULL OR status IN ('open', 'waiting', 'done', 'dropped')),
         priority TEXT CHECK(priority IS NULL OR priority IN ('S', 'A', 'B', 'C', 'D')),
         scheduled TEXT CHECK(scheduled IS NULL OR
             (length(scheduled) = 10 AND date(scheduled, '+0 days') IS scheduled)),
         due TEXT CHECK(due IS NULL OR
             (length(due) = 10 AND date(due, '+0 days') IS due)),
         closed TEXT CHECK(closed IS NULL OR
             (length(closed) = 20 AND
              strftime('%Y-%m-%dT%H:%M:%SZ', closed) IS closed)),
         CHECK(length(created) = 20 AND
             strftime('%Y-%m-%dT%H:%M:%SZ', created) IS created),
         CHECK(length(updated) = 20 AND
             strftime('%Y-%m-%dT%H:%M:%SZ', updated) IS updated),
         CHECK(kind = 'todo' OR
             (status IS NULL AND priority IS NULL AND scheduled IS NULL AND
              due IS NULL AND closed IS NULL)),
         CHECK((status IN ('done', 'dropped') AND closed IS NOT NULL) OR
             ((status IS NULL OR status IN ('open', 'waiting')) AND closed IS NULL)),
         UNIQUE(id, home_collection_id),
         FOREIGN KEY(id, home_collection_id)
             REFERENCES note_collections(note_id, collection_id)
             DEFERRABLE INITIALLY DEFERRED
     )",
    "CREATE TABLE note_collections (
         note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
         collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE RESTRICT,
         PRIMARY KEY(note_id, collection_id)
     )",
    "CREATE TABLE note_tags (
         note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
         tag TEXT NOT NULL,
         PRIMARY KEY(note_id, tag)
     )",
    "CREATE TABLE note_sources (
         note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
         source TEXT NOT NULL,
         PRIMARY KEY(note_id, source)
     )",
    "CREATE TABLE note_links (
         note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
         target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
         PRIMARY KEY(note_id, target_id)
     )",
    "CREATE TABLE note_search_rows (
         search_id INTEGER PRIMARY KEY,
         note_id TEXT NOT NULL UNIQUE REFERENCES notes(id) ON DELETE CASCADE
     )",
    "CREATE VIRTUAL TABLE note_fts USING fts5(
         title,
         body,
         content = '',
         tokenize = 'unicode61 remove_diacritics 2'
     )",
    "CREATE TRIGGER notes_search_insert
     AFTER INSERT ON notes BEGIN
         INSERT INTO note_search_rows (note_id) VALUES (new.id);
         INSERT INTO note_fts (rowid, title, body)
         SELECT search_id, new.title, new.body
         FROM note_search_rows WHERE note_id = new.id;
     END",
    "CREATE TRIGGER notes_search_update
     AFTER UPDATE OF title, body ON notes BEGIN
         INSERT INTO note_fts (note_fts, rowid, title, body)
         SELECT 'delete', search_id, old.title, old.body
         FROM note_search_rows WHERE note_id = old.id;
         INSERT INTO note_fts (rowid, title, body)
         SELECT search_id, new.title, new.body
         FROM note_search_rows WHERE note_id = new.id;
     END",
    "CREATE TRIGGER notes_search_delete
     BEFORE DELETE ON notes BEGIN
         INSERT INTO note_fts (note_fts, rowid, title, body)
         SELECT 'delete', search_id, old.title, old.body
         FROM note_search_rows WHERE note_id = old.id;
     END",
    "CREATE INDEX notes_created ON notes(created DESC, id DESC)",
    "CREATE INDEX notes_open_todos_created
         ON notes(created DESC, id DESC)
         WHERE kind = 'todo' AND status = 'open'",
    "CREATE INDEX notes_status_created
         ON notes(LOWER(status), created DESC, id DESC)
         WHERE status IS NOT NULL",
    "CREATE INDEX notes_created_day
         ON notes(substr(created, 1, 10), created DESC, id DESC)",
    "CREATE INDEX notes_id_nocase ON notes(id COLLATE NOCASE)",
    "CREATE INDEX note_collections_collection ON note_collections(collection_id)",
    "CREATE INDEX note_tags_lower_tag_note ON note_tags(LOWER(tag), note_id)",
    "CREATE INDEX note_links_target ON note_links(target_id)",
    "INSERT INTO schema_version (singleton, version) VALUES (1, 3)",
];

const VERSION_TABLE_MIGRATION_STEPS: &[&str] = &[
    "ALTER TABLE schema_version RENAME TO schema_version_legacy",
    "CREATE TABLE schema_version (
         singleton INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL
     ) WITHOUT ROWID",
    "INSERT INTO schema_version (singleton, version)
         SELECT 1, version FROM schema_version_legacy",
    "DROP TABLE schema_version_legacy",
];

const REQUIRED_OBJECTS: &[(&str, &str)] = &[
    ("table", "schema_version"),
    ("table", "vaults"),
    ("table", "collections"),
    ("table", "notes"),
    ("table", "note_collections"),
    ("table", "note_tags"),
    ("table", "note_sources"),
    ("table", "note_links"),
    ("table", "note_search_rows"),
    ("table", "note_fts"),
    ("trigger", "notes_search_insert"),
    ("trigger", "notes_search_update"),
    ("trigger", "notes_search_delete"),
    ("index", "notes_created"),
    ("index", "notes_open_todos_created"),
    ("index", "notes_status_created"),
    ("index", "notes_created_day"),
    ("index", "notes_id_nocase"),
    ("index", "note_collections_collection"),
    ("index", "note_tags_lower_tag_note"),
    ("index", "note_links_target"),
];

pub(super) fn configure_and_initialize(connection: &mut Connection) -> Result<()> {
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;",
    )?;

    if has_schema_version(connection)? && is_singleton_version_table(connection)? {
        return validate_schema(connection);
    }
    initialize_with(connection, |_| Ok(()))
}

pub(super) fn configure_existing(connection: &Connection) -> Result<()> {
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

fn initialize_with(
    connection: &mut Connection,
    mut after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if has_schema_version(&transaction)? {
        validate_declared_version(&transaction)?;
        if is_singleton_version_table(&transaction)? {
            validate_schema(&transaction)?;
        } else {
            validate_required_objects(&transaction)?;
            for (step, sql) in VERSION_TABLE_MIGRATION_STEPS.iter().enumerate() {
                transaction.execute_batch(sql)?;
                after_step(step)?;
            }
            validate_schema(&transaction)?;
        }
    } else {
        for (step, sql) in SCHEMA_STEPS.iter().enumerate() {
            transaction.execute_batch(sql)?;
            after_step(step)?;
        }
        validate_schema(&transaction)?;
    }
    transaction.commit()?;
    Ok(())
}

fn has_schema_version(connection: &Connection) -> Result<bool> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn validate_schema(connection: &Connection) -> Result<()> {
    validate_declared_version(connection)?;
    if !is_singleton_version_table(connection)? {
        return invalid_schema("schema_version is not a constrained singleton table");
    }
    let singleton: i64 = connection.query_row(
        "SELECT singleton FROM schema_version WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    if singleton != 1 {
        return invalid_schema("schema_version has an invalid singleton key");
    }
    validate_required_objects(connection)
}

fn validate_declared_version(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("SELECT version FROM schema_version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match versions.as_slice() {
        [SCHEMA_VERSION] => {}
        [version] => {
            return Err(NtError::Message(format!(
                "unsupported database schema version {version}"
            )));
        }
        _ => return invalid_schema("schema_version must contain exactly one row"),
    }
    Ok(())
}

fn is_singleton_version_table(connection: &Connection) -> Result<bool> {
    let version_table_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    Ok(version_table_sql
        .to_ascii_lowercase()
        .contains("check (singleton = 1)"))
}

fn validate_required_objects(connection: &Connection) -> Result<()> {
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
            return invalid_schema(&format!("missing required {object_type} {name}"));
        }
    }

    let fts_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'note_fts'",
        [],
        |row| row.get(0),
    )?;
    if !fts_sql.to_ascii_lowercase().contains("using fts5") {
        return invalid_schema("note_fts is not an FTS5 virtual table");
    }
    Ok(())
}

fn invalid_schema<T>(detail: &str) -> Result<T> {
    Err(NtError::Message(format!(
        "invalid database schema: {detail}"
    )))
}

pub(super) fn is_nt_database(connection: &Connection) -> Result<bool> {
    if !has_schema_version(connection)? {
        return Ok(false);
    }
    if is_singleton_version_table(connection)? {
        validate_schema(connection)?;
    } else {
        validate_declared_version(connection)?;
        validate_required_objects(connection)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_open_todo(connection: &mut Connection) {
        let transaction = connection.transaction().unwrap();
        transaction
            .execute_batch(
                "INSERT INTO vaults (id, name, created)
                     VALUES ('vault', 'personal', '2026-05-28T14:30:12Z');
                 INSERT INTO collections (id, vault_id, name, created)
                     VALUES ('inbox', 'vault', 'inbox', '2026-05-28T14:30:12Z');
                 INSERT INTO notes
                     (id, home_collection_id, body, created, updated, title, kind,
                      status, priority, scheduled, due, closed)
                     VALUES ('todo', 'inbox', '# Todo', '2026-05-28T14:30:12Z',
                             '2026-05-28T14:30:12Z', 'Todo', 'todo', 'open', 'A',
                             '2026-05-29', '2026-05-30', NULL);
                 INSERT INTO note_collections (note_id, collection_id)
                     VALUES ('todo', 'inbox');",
            )
            .unwrap();
        transaction.commit().unwrap();
    }

    #[test]
    fn every_failed_initialization_step_rolls_back_the_entire_schema() {
        for failed_step in 0..SCHEMA_STEPS.len() {
            let mut connection = Connection::open_in_memory().unwrap();
            let result = initialize_with(&mut connection, |step| {
                if step == failed_step {
                    return Err(NtError::Message("injected migration failure".into()));
                }
                Ok(())
            });

            assert!(result.is_err(), "step {failed_step} unexpectedly succeeded");
            let object_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(object_count, 0, "step {failed_step} left schema objects");
        }
    }

    #[test]
    fn every_failed_version_table_migration_step_preserves_the_old_schema() {
        for failed_step in 0..VERSION_TABLE_MIGRATION_STEPS.len() {
            let mut connection = Connection::open_in_memory().unwrap();
            initialize_with(&mut connection, |_| Ok(())).unwrap();
            connection
                .execute_batch(
                    "ALTER TABLE schema_version RENAME TO schema_version_new;
                     CREATE TABLE schema_version (version INTEGER NOT NULL);
                     INSERT INTO schema_version (version)
                         SELECT version FROM schema_version_new;
                     DROP TABLE schema_version_new;",
                )
                .unwrap();

            let result = initialize_with(&mut connection, |step| {
                if step == failed_step {
                    return Err(NtError::Message("injected migration failure".into()));
                }
                Ok(())
            });

            assert!(result.is_err(), "step {failed_step} unexpectedly succeeded");
            assert!(!is_singleton_version_table(&connection).unwrap());
            validate_declared_version(&connection).unwrap();
            validate_required_objects(&connection).unwrap();
        }
    }

    #[test]
    fn existing_version_requires_every_schema_object() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_with(&mut connection, |_| Ok(())).unwrap();
        connection.execute("DROP INDEX notes_created", []).unwrap();

        let error = initialize_with(&mut connection, |_| Ok(())).unwrap_err();

        assert_eq!(
            error.to_string(),
            "invalid database schema: missing required index notes_created"
        );
    }

    #[test]
    fn schema_version_rejects_extra_rows() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_with(&mut connection, |_| Ok(())).unwrap();

        let error = connection
            .execute(
                "INSERT INTO schema_version (singleton, version) VALUES (2, 3)",
                [],
            )
            .unwrap_err();

        assert!(error.to_string().contains("CHECK constraint failed"));
    }

    #[test]
    fn notes_reject_invalid_metadata_domains_and_shapes() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_with(&mut connection, |_| Ok(())).unwrap();
        insert_open_todo(&mut connection);

        for sql in [
            "UPDATE notes SET status = 'active' WHERE id = 'todo'",
            "UPDATE notes SET priority = 'urgent' WHERE id = 'todo'",
            "UPDATE notes SET scheduled = '2026/05/29' WHERE id = 'todo'",
            "UPDATE notes SET due = '2026-02-29' WHERE id = 'todo'",
            "UPDATE notes SET created = '2026-05-28' WHERE id = 'todo'",
            "UPDATE notes SET updated = '2026-05-28T25:00:00Z' WHERE id = 'todo'",
            "UPDATE notes SET closed = '2026-05-28' WHERE id = 'todo'",
            "UPDATE notes SET closed = '2026-05-28T15:00:00Z' WHERE id = 'todo'",
            "UPDATE notes SET status = 'done' WHERE id = 'todo'",
            "UPDATE notes SET kind = 'note' WHERE id = 'todo'",
        ] {
            assert!(connection.execute(sql, []).is_err(), "accepted `{sql}`");
        }

        connection
            .execute(
                "UPDATE notes SET kind = 'note', status = NULL, priority = NULL,
                 scheduled = NULL, due = NULL, closed = NULL WHERE id = 'todo'",
                [],
            )
            .unwrap();
        for sql in [
            "UPDATE notes SET status = 'open' WHERE id = 'todo'",
            "UPDATE notes SET priority = 'A' WHERE id = 'todo'",
            "UPDATE notes SET scheduled = '2026-05-29' WHERE id = 'todo'",
            "UPDATE notes SET due = '2026-05-30' WHERE id = 'todo'",
            "UPDATE notes SET closed = '2026-05-28T15:00:00Z' WHERE id = 'todo'",
        ] {
            assert!(connection.execute(sql, []).is_err(), "accepted `{sql}`");
        }
    }
}
