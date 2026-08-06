use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use rusqlite::Connection;

fn run(home: &Path, arguments: &[&str], stdin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nt"));
    command.env("HOME", home).args(arguments);
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(stdin) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn success(home: &Path, arguments: &[&str], stdin: Option<&str>) -> String {
    let output = run(home, arguments, stdin);
    assert!(
        output.status.success(),
        "nt {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn add(home: &Path, body: &str, metadata: &[&str]) -> String {
    let mut arguments = vec!["add"];
    arguments.extend_from_slice(metadata);
    success(home, &arguments, Some(body))
        .trim()
        .strip_prefix("saved ")
        .unwrap()
        .to_string()
}

#[test]
fn complete_cli_workflow_matches_the_stable_contract() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(success(home.path(), &["init"], None), "initialized\n");
    assert_eq!(
        success(home.path(), &["init"], None),
        "already initialized\n"
    );

    let target = add(home.path(), "# Target", &[]);
    let source = add(
        home.path(),
        "# Storage\n\nCafé ownership.",
        &["collection:work/nt", "tag:rust,sqlite"],
    );
    assert_eq!(
        success(home.path(), &["show", &source], None),
        "# Storage\n\nCafé ownership."
    );
    assert_eq!(
        success(home.path(), &["tag", &source, "+rust"], None),
        format!("tagged {source} +rust\n")
    );
    assert_eq!(
        success(home.path(), &["move", &source, "research/sqlite"], None),
        format!("moved {source} research/sqlite\n")
    );
    assert_eq!(
        success(home.path(), &["link", &source, &format!("+{target}")], None),
        format!("linked {source} +{target}\n")
    );
    assert_eq!(
        success(
            home.path(),
            &["edit", &source],
            Some("# Updated\n\nBorrowing café storage."),
        ),
        format!("updated {source}\n")
    );

    let row = success(
        home.path(),
        &[
            "list",
            "collection:research/sqlite",
            "tag:rust",
            &format!("link:{target}"),
        ],
        None,
    );
    assert_summary_row(
        &row,
        &source,
        "research/sqlite",
        "Updated",
        &["rust", "sqlite"],
    );
    let found = success(home.path(), &["find", "borrowing cafe", "tag:rust"], None);
    assert_summary_row(
        &found,
        &source,
        "research/sqlite",
        "Updated",
        &["rust", "sqlite"],
    );

    assert_eq!(success(home.path(), &["rm", &target], None), "removed 1\n");
    let connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    let links: i64 = connection
        .query_row("SELECT COUNT(*) FROM note_links", [], |row| row.get(0))
        .unwrap();
    let foreign_key_errors: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((links, foreign_key_errors), (0, 0));
}

#[test]
fn missing_storage_and_atomic_remove_leave_state_unchanged() {
    let home = tempfile::tempdir().unwrap();
    let missing = "018fbe0a-6c00-7000-8000-000000000001";
    let output = run(home.path(), &["show", missing], None);
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"error: run nt init first\n");
    assert!(!home.path().join(".nt").exists());

    success(home.path(), &["init"], None);
    let existing = add(home.path(), "# Keep", &[]);
    let output = run(home.path(), &["rm", &existing, missing], None);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("error: note not found: {missing}\n")
    );
    assert_eq!(success(home.path(), &["show", &existing], None), "# Keep");
}

#[test]
fn wal_readers_keep_their_snapshot_while_a_writer_commits() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    add(home.path(), "# First", &[]);
    let path = home.path().join(".nt/nt.sqlite3");
    let mut reader = Connection::open(path).unwrap();
    let transaction = reader.transaction().unwrap();
    let before: i64 = transaction
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    add(home.path(), "# Second", &[]);
    let snapshot: i64 = transaction
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!((before, snapshot), (1, 1));
    transaction.commit().unwrap();
    let after: i64 = reader
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after, 2);
}

#[test]
fn trailing_body_arguments_accept_empty_redirected_stdin() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    let output = success(
        home.path(),
        &["add", "tag:argument", "--", "# Argument body"],
        None,
    );
    let id = output.trim().strip_prefix("saved ").unwrap();
    assert_eq!(success(home.path(), &["show", id], None), "# Argument body");
}

fn assert_summary_row(
    row: &str,
    expected_id: &str,
    expected_collection: &str,
    expected_title: &str,
    expected_tags: &[&str],
) {
    assert_eq!(row.lines().count(), 1);
    let cells = row.trim_end().split('\t').collect::<Vec<_>>();
    assert_eq!(cells.len(), 5);
    assert_eq!(
        serde_json::from_str::<String>(cells[0]).unwrap(),
        expected_id
    );
    let updated = serde_json::from_str::<String>(cells[1]).unwrap();
    assert_eq!(updated.len(), 20);
    assert_eq!(
        serde_json::from_str::<String>(cells[2]).unwrap(),
        expected_collection
    );
    assert_eq!(
        serde_json::from_str::<String>(cells[3]).unwrap(),
        expected_title
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(cells[4]).unwrap(),
        expected_tags
    );
}
