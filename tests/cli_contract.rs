use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use rusqlite::{Connection, params};

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

fn seed_matching_notes(home: &Path, count: usize) {
    let mut connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let transaction = connection.transaction().unwrap();
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                 VALUES (?1, 'inbox', ?2, ?3, '2026-01-01T00:00:00Z',
                         '2026-01-01T00:00:00Z')",
            )
            .unwrap();
        for index in 0..count {
            statement
                .execute(params![
                    format!("018fbe0a-6c00-7000-8000-{index:012x}"),
                    format!("# Note {index}\nrust streaming"),
                    format!("Note {index}"),
                ])
                .unwrap();
        }
    }
    transaction.commit().unwrap();
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

struct RestorePermissions {
    path: PathBuf,
    permissions: fs::Permissions,
}

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.permissions.clone());
    }
}

#[cfg(unix)]
#[test]
fn init_creates_private_canonical_storage_without_changing_home() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().unwrap();
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o751)).unwrap();

    assert_eq!(success(home.path(), &["init"], None), "initialized\n");
    assert_eq!(mode(home.path()), 0o751);
    assert_eq!(mode(&home.path().join(".nt")), 0o700);
    assert_eq!(mode(&home.path().join(".nt/nt.sqlite3")), 0o600);
}

#[test]
fn invalid_home_values_cannot_create_working_directory_storage() {
    for (home, user_profile) in [
        (Some(""), None),
        (Some("relative/home"), None),
        (Some(""), Some("relative/home")),
        (None, Some("")),
        (None, None),
    ] {
        let working_directory = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_nt"));
        command.current_dir(working_directory.path()).arg("init");
        match home {
            Some(value) => {
                command.env("HOME", value);
            }
            None => {
                command.env_remove("HOME");
            }
        }
        match user_profile {
            Some(value) => {
                command.env("USERPROFILE", value);
            }
            None => {
                command.env_remove("USERPROFILE");
            }
        }
        let output = command.output().unwrap();

        assert!(!output.status.success());
        assert_eq!(output.stdout, b"");
        assert_eq!(output.stderr, b"error: home directory not found\n");
        assert_eq!(working_directory.path().read_dir().unwrap().count(), 0);
    }
}

#[test]
fn read_only_commands_work_on_non_writable_databases() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    let id = add(home.path(), "# Read only", &["tag:rust"]);
    let database = home.path().join(".nt/nt.sqlite3");
    let original_permissions = fs::metadata(&database).unwrap().permissions();
    let _restore = RestorePermissions {
        path: database.clone(),
        permissions: original_permissions.clone(),
    };
    let mut permissions = original_permissions;
    permissions.set_readonly(true);
    fs::set_permissions(&database, permissions).unwrap();

    assert_eq!(success(home.path(), &["show", &id], None), "# Read only");
    let listed = success(home.path(), &["list", "tag:rust"], None);
    assert_summary_row(&listed, &id, "inbox", "Read only", &["rust"], 0);
    let found = success(home.path(), &["find", "read"], None);
    assert_summary_row(&found, &id, "inbox", "Read only", &["rust"], 0);
    assert_eq!(success(home.path(), &["list", "tags"], None), "\"rust\"\n");
    assert_eq!(
        success(home.path(), &["list", "collections"], None),
        "\"inbox\"\n"
    );
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
            &format!("links-to:{target}"),
        ],
        None,
    );
    assert_summary_row(
        &row,
        &source,
        "research/sqlite",
        "Updated",
        &["rust", "sqlite"],
        1,
    );
    let found = success(home.path(), &["find", "borrowing cafe", "tag:rust"], None);
    assert_summary_row(
        &found,
        &source,
        "research/sqlite",
        "Updated",
        &["rust", "sqlite"],
        1,
    );
    let linked_target = success(
        home.path(),
        &["list", &format!("linked-from:{source}")],
        None,
    );
    assert_summary_row(&linked_target, &target, "inbox", "Target", &[], 0);
    let found_target = success(
        home.path(),
        &["find", "target", &format!("linked-from:{source}")],
        None,
    );
    assert_summary_row(&found_target, &target, "inbox", "Target", &[], 0);
    assert_eq!(
        success(home.path(), &["list", "tags"], None),
        "\"rust\"\n\"sqlite\"\n"
    );
    assert_eq!(
        success(home.path(), &["list", "collections"], None),
        "\"inbox\"\n\"research/sqlite\"\n"
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
fn library_cli_preserves_captures_summaries_search_and_note_references() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    let note_id = add(home.path(), "# Synthesis\nUses external evidence.", &[]);
    let added = success(
        home.path(),
        &[
            "library",
            "add",
            "https://example.com/article",
            "Example",
            "Article",
        ],
        Some("Café historical evidence"),
    );
    let library_id = added
        .trim()
        .strip_prefix("library saved ")
        .unwrap()
        .to_string();
    assert_eq!(
        success(
            home.path(),
            &[
                "library",
                "add",
                "https://example.com/article",
                "Changed",
                "Title"
            ],
            Some("Café historical evidence"),
        ),
        format!("library unchanged {library_id}\n")
    );
    assert_eq!(
        success(
            home.path(),
            &["library", "summary", &library_id],
            Some("First summary")
        ),
        format!("summarized {library_id}\n")
    );
    assert_eq!(
        success(home.path(), &["ref", &note_id, &library_id], None),
        format!("referenced {note_id} {library_id}\n")
    );
    assert_eq!(
        success(
            home.path(),
            &["library", "capture", &library_id],
            Some("Current résumé evidence")
        ),
        format!("captured {library_id}\n")
    );
    assert_eq!(
        success(home.path(), &["library", "show", &library_id], None),
        "Current résumé evidence"
    );
    assert!(success(home.path(), &["library", "find", "historical"], None).is_empty());
    let found = success(home.path(), &["library", "find", "resume"], None);
    let cells = found.trim_end().split('\t').collect::<Vec<_>>();
    assert_eq!(cells.len(), 7);
    assert_eq!(
        serde_json::from_str::<String>(cells[0]).unwrap(),
        library_id
    );
    assert_eq!(
        serde_json::from_str::<String>(cells[1]).unwrap(),
        "https://example.com/article"
    );
    assert_eq!(
        serde_json::from_str::<String>(cells[2]).unwrap(),
        "Example Article"
    );
    assert_eq!(serde_json::from_str::<String>(cells[6]).unwrap(), "");

    let history = success(home.path(), &["library", "history", &library_id], None);
    assert_eq!(history.lines().count(), 2);
    assert!(history.lines().next().unwrap().contains("First summary"));
    let connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    let refs: i64 = connection
        .query_row("SELECT COUNT(*) FROM note_library_refs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(refs, 1);
    drop(connection);
    assert_eq!(
        success(home.path(), &["unref", &note_id, &library_id], None),
        format!("unreferenced {note_id} {library_id}\n")
    );
}

#[test]
fn missing_storage_and_atomic_remove_leave_state_unchanged() {
    let home = tempfile::tempdir().unwrap();
    let missing = "018fbe0a-6c00-7000-8000-000000000001";
    let output = run(home.path(), &["show", missing], None);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
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
fn process_exit_codes_distinguish_input_and_operational_errors() {
    let home = tempfile::tempdir().unwrap();
    let syntax = run(home.path(), &["show"], None);
    assert_eq!(syntax.status.code(), Some(2));

    success(home.path(), &["init"], None);
    let invalid_query = run(home.path(), &["list", "unknown:value"], None);
    assert_eq!(invalid_query.status.code(), Some(2));

    let missing = run(
        home.path(),
        &["show", "018fbe0a-6c00-7000-8000-000000000001"],
        None,
    );
    assert_eq!(missing.status.code(), Some(3));
}

#[test]
fn invalid_persisted_values_are_operational_errors() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    add(home.path(), "# Invalid stored value", &[]);
    Connection::open(home.path().join(".nt/nt.sqlite3"))
        .unwrap()
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE notes SET collection = 'Invalid';
             PRAGMA ignore_check_constraints = OFF;",
        )
        .unwrap();

    let output = run(home.path(), &["list", "collections"], None);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: stored note is invalid (row: 1, field: collection)\n"
    );
}

#[test]
fn multi_megabyte_bodies_round_trip_through_capture_edit_and_find() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    let body = format!("# Large\nneedle {}", "x".repeat(2 * 1024 * 1024));
    let id = add(home.path(), &body, &[]);

    assert_eq!(success(home.path(), &["show", &id], None), body);
    assert_summary_row(
        &success(home.path(), &["find", "needle"], None),
        &id,
        "inbox",
        "Large",
        &[],
        0,
    );

    let edited = format!(
        "# Edited large\nreplacement {}",
        "y".repeat(2 * 1024 * 1024)
    );
    success(home.path(), &["edit", &id], Some(&edited));
    assert_eq!(success(home.path(), &["show", &id], None), edited);
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

#[test]
fn find_exits_cleanly_when_a_pipe_consumer_closes_early() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    seed_matching_notes(home.path(), 5000);
    Connection::open(home.path().join(".nt/nt.sqlite3"))
        .unwrap()
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             INSERT INTO notes(id, collection, body, title, created, updated)
             VALUES ('malformed', 'inbox', '# Invalid\nrust streaming', 'Invalid',
                     '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z');
             PRAGMA ignore_check_constraints = OFF;",
        )
        .unwrap();

    let mut nt = Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home.path())
        .args(["find", "rust"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(nt.stdout.take().unwrap());
    for _ in 0..100 {
        let mut line = String::new();
        assert_ne!(output.read_line(&mut line).unwrap(), 0);
    }
    drop(output);
    let output = nt.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn redirected_summaries_escape_titles_without_creating_extra_lines() {
    let home = tempfile::tempdir().unwrap();
    success(home.path(), &["init"], None);
    let title = "A\t\"quoted\" \\ title";
    let id = add(home.path(), &format!("# {title}\nBody"), &[]);

    let output = success(home.path(), &["list"], None);

    assert_summary_row(output.trim_end(), &id, "inbox", title, &[], 0);
}

fn assert_summary_row(
    row: &str,
    expected_id: &str,
    expected_collection: &str,
    expected_title: &str,
    expected_tags: &[&str],
    expected_outgoing: u64,
) {
    assert_eq!(row.lines().count(), 1);
    let cells = row.trim_end().split('\t').collect::<Vec<_>>();
    assert_eq!(cells.len(), 6);
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
    assert_eq!(
        serde_json::from_str::<u64>(cells[5]).unwrap(),
        expected_outgoing
    );
}
