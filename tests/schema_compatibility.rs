use std::path::Path;

use nt::Repository;
use rusqlite::Connection;

const V1_SCHEMA: &str = include_str!("fixtures/v1_schema.sql");

#[test]
fn initialized_schema_matches_the_independent_v1_fixture() {
    let directory = tempfile::tempdir().unwrap();
    let fixture_path = directory.path().join("fixture.sqlite3");
    let initialized_path = directory.path().join("initialized.sqlite3");

    let fixture = Connection::open(&fixture_path).unwrap();
    let fixture_schema = V1_SCHEMA.replace("\r\n", "\n").replace('\r', "\n");
    fixture.execute_batch(&fixture_schema).unwrap();
    drop(fixture);

    Repository::initialize_at(&initialized_path).unwrap();

    assert_database_identity(&fixture_path);
    assert_database_identity(&initialized_path);
    assert_eq!(
        schema_entries(&initialized_path),
        schema_entries(&fixture_path)
    );
    assert!(Repository::open_read_only(&fixture_path).is_ok());
}

fn assert_database_identity(path: &Path) {
    let connection = Connection::open(path).unwrap();
    let application_id: i64 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .unwrap();
    let version: i64 = connection
        .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(application_id, 0x4e54_4e54);
    assert_eq!(version, 1);
}

fn schema_entries(path: &Path) -> Vec<(String, String, Option<String>)> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT type, name, sql
             FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
             ORDER BY rowid",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
