mod common;

use std::fs;

use common::{assert_failed, assert_uuid_v7, run_nt, temp_dir};
use rusqlite::Connection;

#[test]
fn init_creates_logical_vault_and_inbox_in_one_database() {
    let root = temp_dir("logical-init");
    let home = root.join("home");

    assert_eq!(
        run_nt(&home, &["init", "personal"]).trim(),
        "initialized personal"
    );
    assert!(!root.join("personal").exists());

    let database = home.join(".nt/nt.sqlite3");
    assert!(database.is_file());

    let connection = Connection::open(database).unwrap();
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode, "wal");
    let (vault_id, vault_name): (String, String) = connection
        .query_row("SELECT id, name FROM vaults", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_uuid_v7(&vault_id);
    assert_eq!(vault_name, "personal");

    let (collection_id, collection_name): (String, String) = connection
        .query_row("SELECT id, name FROM collections", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_uuid_v7(&collection_id);
    assert_eq!(collection_name, "inbox");
    let schema_version: i64 = connection
        .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(schema_version, 2);
    let search_table: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name = 'note_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(search_table.contains("fts5"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_refuses_to_modify_an_existing_unrelated_database() {
    let root = temp_dir("init-existing-database");
    let home = root.join("home");
    let nt_home = home.join(".nt");
    fs::create_dir_all(&nt_home).unwrap();
    let database = nt_home.join("nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch("CREATE TABLE sentinel (value TEXT); INSERT INTO sentinel VALUES ('kept');")
        .unwrap();
    drop(connection);

    assert_failed(&home, &["init", "personal"], "", "refusing to overwrite it");

    let connection = Connection::open(database).unwrap();
    let sentinel: String = connection
        .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sentinel, "kept");
    let nt_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'vaults'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(nt_tables, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn unsupported_schema_version_is_rejected_without_modification() {
    let root = temp_dir("old-schema-recreate");
    let home = root.join("home");
    let nt_home = home.join(".nt");
    fs::create_dir_all(&nt_home).unwrap();
    let database = nt_home.join("nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL); INSERT INTO schema_version VALUES (1);").unwrap();
    drop(connection);

    assert_failed(
        &home,
        &["find", "body:anything"],
        "",
        "unsupported database schema version 1",
    );

    let connection = Connection::open(database).unwrap();
    let fts_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'note_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts_count, 0);
    let _ = fs::remove_dir_all(root);
}
