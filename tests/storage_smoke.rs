use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rusqlite::Connection;
use uuid::{Uuid, Version};

fn nt_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_nt") {
        return PathBuf::from(path);
    }
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("nt");
    path
}

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
    assert!(!home.join(".nt/index.json").exists());

    let connection = Connection::open(database).unwrap();
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn note_body_and_metadata_are_canonical_in_sqlite() {
    let root = temp_dir("sqlite-body");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);

    let saved = run_nt_with_stdin(
        &home,
        &["note", "tag:rust"],
        "# Rust ownership\n\nBorrow checker notes.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();
    assert_uuid_v7(id);

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let (body, title, home_collection): (String, String, String) = connection
        .query_row(
            "SELECT n.body, n.title, v.name || '/' || c.name
             FROM notes n
             JOIN collections c ON c.id = n.home_collection_id
             JOIN vaults v ON v.id = c.vault_id
             WHERE n.id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(body, "# Rust ownership\n\nBorrow checker notes.\n");
    assert_eq!(title, "Rust ownership");
    assert_eq!(home_collection, "personal/inbox");

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("home personal/inbox"));
    assert!(shown.contains("kind note\n"));
    for todo_field in ["status", "priority", "scheduled", "due", "closed"] {
        assert!(!shown.contains(&format!("\n{todo_field} ")));
    }
    assert!(shown.ends_with("# Rust ownership\n\nBorrow checker notes.\n"));
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:borrow"])),
        vec![id]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn todo_show_includes_todo_metadata() {
    let root = temp_dir("todo-show-metadata");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);

    let saved = run_nt_with_stdin(
        &home,
        &[
            "todo",
            "status:waiting",
            "priority:A",
            "scheduled:2026-08-01",
            "due:2026-08-02",
        ],
        "# Ship release\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    for metadata in [
        "kind todo",
        "status waiting",
        "priority A",
        "scheduled 2026-08-01",
        "due 2026-08-02",
        "closed -",
    ] {
        assert!(shown.contains(&format!("\n{metadata}\n")));
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn notes_can_reference_collections_across_logical_vaults() {
    let root = temp_dir("cross-vault-membership");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    run_nt(&home, &["init", "work"]);

    assert_failed(
        &home,
        &["note"],
        "# Ambiguous\n",
        "specify `home:<vault>/<collection>`",
    );
    let saved = run_nt_with_stdin(
        &home,
        &["note", "home:personal/rust", "collection:work/project_a"],
        "# Shared knowledge\n\nPortable context.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("home personal/rust"));
    assert!(shown.contains("collections personal/rust,work/project_a"));
    assert_eq!(
        run_nt(&home, &["list", "collections"]),
        "personal/inbox\npersonal/rust\nwork/inbox\nwork/project_a\n"
    );

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let memberships: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM note_collections WHERE note_id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(memberships, 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn home_collection_is_a_required_membership_and_can_move() {
    let root = temp_dir("move-home");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    run_nt(&home, &["init", "work"]);
    let saved = run_nt_with_stdin(&home, &["note", "home:personal/rust"], "# Move me\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();

    assert_failed(
        &home,
        &["update", id, "collection", "-personal/rust"],
        "",
        "cannot remove home collection",
    );
    run_nt(&home, &["update", id, "home", "work/project_a"]);
    run_nt(&home, &["update", id, "collection", "-personal/rust"]);

    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("home work/project_a"));
    assert!(shown.contains("collections work/project_a"));

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let foreign_key_errors: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(foreign_key_errors, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn remove_is_transactional_and_cleans_relationships() {
    let root = temp_dir("remove-relations");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(&home, &["note"], "# First\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["note", &format!("link:{first_id}")], "# Second\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();

    run_nt(&home, &["rm", first_id]);
    let shown = run_nt(&home, &["show", second_id]);
    assert!(shown.contains("links -"));

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let note_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM note_links", [], |row| row.get(0))
        .unwrap();
    assert_eq!((note_count, link_count), (1, 0));

    let _ = fs::remove_dir_all(root);
}

fn assert_uuid_v7(value: &str) {
    let uuid = Uuid::parse_str(value).unwrap();
    assert_eq!(uuid.get_version(), Some(Version::SortRand));
    assert_eq!(uuid.to_string(), value);
}

fn summary_ids(output: &str) -> Vec<&str> {
    output
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect()
}

fn run_nt(home: &Path, args: &[&str]) -> String {
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

fn run_nt_with_stdin(home: &Path, args: &[&str], stdin: &str) -> String {
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

fn assert_failed(home: &Path, args: &[&str], stdin: &str, expected: &str) {
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

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
