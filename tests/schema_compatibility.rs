use std::fs;
use std::path::Path;
use std::process::Command;

use rusqlite::Connection;

const V5_SCHEMA: &str = include_str!("fixtures/v5_schema.sql");

#[test]
fn initialized_schema_matches_the_independent_v5_fixture() {
    let directory = tempfile::tempdir().unwrap();
    let fixture_home = directory.path().join("fixture-home");
    let initialized_home = directory.path().join("initialized-home");
    fs::create_dir_all(fixture_home.join(".nt")).unwrap();
    fs::create_dir_all(&initialized_home).unwrap();
    let fixture_path = fixture_home.join(".nt/nt.sqlite3");
    let initialized_path = initialized_home.join(".nt/nt.sqlite3");

    let fixture = Connection::open(&fixture_path).unwrap();
    let fixture_schema = V5_SCHEMA.replace("\r\n", "\n").replace('\r', "\n");
    fixture.execute_batch(&fixture_schema).unwrap();
    drop(fixture);

    let initialized = nt(&initialized_home, &["init"]);
    assert!(initialized.status.success());
    assert_eq!(initialized.stdout, b"initialized\n");
    assert!(initialized.stderr.is_empty());

    assert_database_identity(&fixture_path);
    assert_database_identity(&initialized_path);
    assert_eq!(
        schema_entries(&initialized_path),
        schema_entries(&fixture_path)
    );
    let validated = nt(&fixture_home, &["list"]);
    assert!(validated.status.success());
    assert!(validated.stdout.is_empty());
    assert!(validated.stderr.is_empty());
    let read = nt(&fixture_home, &["read"]);
    assert!(read.status.success());
    assert!(read.stdout.is_empty());
    assert!(read.stderr.is_empty());
}

fn nt(home: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(arguments)
        .output()
        .unwrap()
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
    assert_eq!(version, 5);
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
