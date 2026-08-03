#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rusqlite::Connection;
use uuid::{Uuid, Version};

pub(crate) fn nt_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_nt") {
        return PathBuf::from(path);
    }
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("nt");
    path
}

#[derive(Debug, PartialEq)]
pub(crate) struct NoteSnapshot {
    pub(crate) row: StoredNoteRow,
    pub(crate) collections: Vec<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) sources: Vec<String>,
    pub(crate) links: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct StoredNoteRow {
    id: String,
    home_collection_id: String,
    body: String,
    created: String,
    updated: String,
    title: String,
    status: Option<String>,
    priority: Option<String>,
    scheduled: Option<String>,
    due: Option<String>,
    closed: Option<String>,
}

pub(crate) fn note_snapshot(connection: &Connection, id: &str) -> NoteSnapshot {
    let row = connection
        .query_row(
            "SELECT n.id, n.home_collection_id, n.body, n.created, n.updated, n.title,
                    n.status, n.priority, n.scheduled, n.due, n.closed
             FROM notes n WHERE n.id = ?1",
            [id],
            |row| {
                Ok(StoredNoteRow {
                    id: row.get(0)?,
                    home_collection_id: row.get(1)?,
                    body: row.get(2)?,
                    created: row.get(3)?,
                    updated: row.get(4)?,
                    title: row.get(5)?,
                    status: row.get(6)?,
                    priority: row.get(7)?,
                    scheduled: row.get(8)?,
                    due: row.get(9)?,
                    closed: row.get(10)?,
                })
            },
        )
        .unwrap();
    NoteSnapshot {
        row,
        collections: query_values(
            connection,
            "SELECT collection_id FROM note_collections WHERE note_id = ?1 ORDER BY collection_id",
            id,
        ),
        tags: query_values(
            connection,
            "SELECT tag FROM note_tags WHERE note_id = ?1 ORDER BY tag",
            id,
        ),
        sources: query_values(
            connection,
            "SELECT source FROM note_sources WHERE note_id = ?1 ORDER BY source",
            id,
        ),
        links: query_values(
            connection,
            "SELECT target_id FROM note_links WHERE note_id = ?1 ORDER BY target_id",
            id,
        ),
    }
}

fn query_values(connection: &Connection, sql: &str, id: &str) -> Vec<String> {
    connection
        .prepare(sql)
        .unwrap()
        .query_map([id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

pub(crate) fn install_note_audit(connection: &Connection, id: &str) {
    connection
        .execute_batch(&format!(
            "CREATE TABLE mutation_audit (event TEXT NOT NULL);
             CREATE TRIGGER audit_note_update AFTER UPDATE ON notes
                 WHEN OLD.id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('note update'); END;
             CREATE TRIGGER audit_note_delete AFTER DELETE ON notes
                 WHEN OLD.id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('note delete'); END;
             CREATE TRIGGER audit_collection_delete AFTER DELETE ON note_collections
                 WHEN OLD.note_id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('collection delete'); END;
             CREATE TRIGGER audit_tag_delete AFTER DELETE ON note_tags
                 WHEN OLD.note_id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('tag delete'); END;
             CREATE TRIGGER audit_source_delete AFTER DELETE ON note_sources
                 WHEN OLD.note_id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('source delete'); END;
             CREATE TRIGGER audit_link_delete AFTER DELETE ON note_links
                 WHEN OLD.note_id = '{id}' BEGIN INSERT INTO mutation_audit VALUES ('link delete'); END;"
        ))
        .unwrap();
}

pub(crate) fn audit_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM mutation_audit", [], |row| row.get(0))
        .unwrap()
}

pub(crate) fn assert_foreign_keys(connection: &Connection) {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

pub(crate) fn assert_search_index_consistent(connection: &Connection) {
    let note_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    let search_row_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM note_search_rows", [], |row| {
            row.get(0)
        })
        .unwrap();
    let fts_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM note_fts", [], |row| row.get(0))
        .unwrap();
    let orphan_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM note_fts
             LEFT JOIN note_search_rows ON note_search_rows.search_id = note_fts.rowid
             WHERE note_search_rows.search_id IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((search_row_count, fts_count), (note_count, note_count));
    assert_eq!(orphan_count, 0);
    connection
        .execute(
            "INSERT INTO note_fts(note_fts) VALUES('integrity-check')",
            [],
        )
        .unwrap();
}

pub(crate) fn assert_uuid_v7(value: &str) {
    let uuid = Uuid::parse_str(value).unwrap();
    assert_eq!(uuid.get_version(), Some(Version::SortRand));
    assert_eq!(uuid.to_string(), value);
}

pub(crate) fn summary_ids(output: &str) -> Vec<&str> {
    output
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect()
}

pub(crate) fn run_nt(home: &Path, args: &[&str]) -> String {
    let output = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "nt {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

pub(crate) fn run_nt_with_stdin(home: &Path, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "nt {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

pub(crate) fn assert_failed(home: &Path, args: &[&str], stdin: &str, expected: &str) {
    let mut child = Command::new(nt_bin())
        .env("HOME", home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}: {stderr}"
    );
}

pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
