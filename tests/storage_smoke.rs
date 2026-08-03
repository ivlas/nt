use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};
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
fn older_development_schema_requires_recreation_without_partial_upgrade() {
    let root = temp_dir("old-schema-recreate");
    let home = root.join("home");
    let nt_home = home.join(".nt");
    fs::create_dir_all(&nt_home).unwrap();
    let database = nt_home.join("nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE schema_version (version INTEGER NOT NULL);
             INSERT INTO schema_version VALUES (1);",
        )
        .unwrap();
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
        run_nt(&home, &["list", "collection"]),
        "personal/rust,work/project_a\n"
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

#[test]
fn inserting_a_note_does_not_rewrite_existing_rows() {
    let root = temp_dir("targeted-insert");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(
        &home,
        &[
            "note",
            "tag:first",
            "collection:personal/archive",
            "source:https://example.com/first",
        ],
        "# First\n\nStable body.\n",
    );
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let database = home.join(".nt/nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    let before = note_snapshot(&connection, first_id);
    install_note_audit(&connection, first_id);
    drop(connection);

    run_nt_with_stdin(&home, &["note", "tag:second"], "# Second\n");

    let connection = Connection::open(database).unwrap();
    assert_eq!(note_snapshot(&connection, first_id), before);
    assert_eq!(audit_count(&connection), 0);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn updating_a_note_does_not_rewrite_unrelated_rows() {
    let root = temp_dir("targeted-update");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(
        &home,
        &["note", "tag:first", "source:https://example.com/first"],
        "# First\n",
    );
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["note", "tag:second"], "# Second\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let database = home.join(".nt/nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    let before = note_snapshot(&connection, first_id);
    install_note_audit(&connection, first_id);
    drop(connection);

    run_nt(&home, &["update", second_id, "tag", "+changed"]);

    let connection = Connection::open(database).unwrap();
    assert_eq!(note_snapshot(&connection, first_id), before);
    assert_eq!(audit_count(&connection), 0);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn removing_valid_and_missing_ids_deletes_nothing() {
    let root = temp_dir("transactional-delete");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(&home, &["note", "tag:keep"], "# Keep first\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    let second = run_nt_with_stdin(&home, &["note"], "# Keep second\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let missing = Uuid::now_v7().to_string();
    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let first_before = note_snapshot(&connection, first_id);
    let second_before = note_snapshot(&connection, second_id);
    drop(connection);

    assert_failed(&home, &["rm", first_id, &missing], "", "note not found");

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    assert_eq!(note_snapshot(&connection, first_id), first_before);
    assert_eq!(note_snapshot(&connection, second_id), second_before);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn open_rejects_a_stale_editor_update() {
    let root = temp_dir("concurrent-open");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(&home, &["note"], "# Concurrent edit\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let editor = root.join("concurrent-editor.sh");
    fs::write(
        &editor,
        "#!/bin/sh\nsleep 1\n\"$NT_BIN\" update \"$NOTE_ID\" tag +concurrent >/dev/null\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(nt_bin())
        .env("HOME", &home)
        .env("EDITOR", &editor)
        .env("NT_BIN", nt_bin())
        .env("NOTE_ID", id)
        .args(["open", id])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("note changed during edit; please retry")
    );
    assert!(run_nt(&home, &["show", id]).contains("tags concurrent"));

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    assert_foreign_keys(&connection);
    assert_search_index_consistent(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn readers_continue_on_the_committed_snapshot_while_a_writer_is_active() {
    let root = temp_dir("wal-reader-writer");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(
        &home,
        &["note"],
        "# Committed Heading\n\nThe committedword is visible.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let database = home.join(".nt/nt.sqlite3");
    let mut writer = Connection::open(&database).unwrap();
    writer.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let transaction = writer
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    transaction
        .execute(
            "UPDATE notes SET title = 'Pending Heading', body = 'pendingword' WHERE id = ?1",
            [id],
        )
        .unwrap();

    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:committedword"])),
        vec![id]
    );
    assert!(run_nt(&home, &["find", "body:pendingword"]).is_empty());

    transaction.commit().unwrap();
    assert!(run_nt(&home, &["find", "body:committedword"]).is_empty());
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:pendingword"])),
        vec![id]
    );
    let connection = Connection::open(database).unwrap();
    assert_search_index_consistent(&connection);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn independent_note_inserts_both_commit() {
    let root = temp_dir("concurrent-inserts");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for (tag, body) in [
        ("tag:parallel_a", "# Parallel A\n\nfirstword\n"),
        ("tag:parallel_b", "# Parallel B\n\nsecondword\n"),
    ] {
        let home = home.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            run_nt_with_stdin(&home, &["note", tag], body)
        }));
    }
    barrier.wait();
    let ids = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .unwrap()
                .trim()
                .strip_prefix("saved ")
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_ne!(ids[0], ids[1]);
    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let note_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(note_count, 2);
    for id in &ids {
        assert_eq!(note_snapshot(&connection, id).tags.len(), 1);
    }
    assert_search_index_consistent(&connection);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn simultaneous_metadata_updates_to_different_notes_both_commit() {
    let root = temp_dir("concurrent-updates");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(&home, &["note"], "# First concurrent update\n");
    let second = run_nt_with_stdin(&home, &["note"], "# Second concurrent update\n");
    let ids = [
        first.trim().strip_prefix("saved ").unwrap().to_string(),
        second.trim().strip_prefix("saved ").unwrap().to_string(),
    ];
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for (id, tag) in ids.iter().cloned().zip(["+parallel_a", "+parallel_b"]) {
        let home = home.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            run_nt(&home, &["update", &id, "tag", tag])
        }));
    }
    barrier.wait();
    for handle in handles {
        handle.join().unwrap();
    }

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    assert_eq!(note_snapshot(&connection, &ids[0]).tags, ["parallel_a"]);
    assert_eq!(note_snapshot(&connection, &ids[1]).tags, ["parallel_b"]);
    assert_search_index_consistent(&connection);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_contention_times_out_cleanly_and_preserves_committed_writes() {
    let root = temp_dir("write-contention");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(&home, &["note"], "# Contended note\n");
    let id = saved.trim().strip_prefix("saved ").unwrap();
    let database = home.join(".nt/nt.sqlite3");
    let mut holder = Connection::open(&database).unwrap();
    holder.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let transaction = holder
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    transaction
        .execute(
            "INSERT INTO note_tags (note_id, tag) VALUES (?1, 'holder')",
            [id],
        )
        .unwrap();

    let mut child = Command::new(nt_bin())
        .env("HOME", &home)
        .args(["update", id, "tag", "+contender"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "contending writer should honor the busy timeout"
    );
    thread::sleep(Duration::from_millis(5_500));
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("database is busy; retry the command")
    );
    transaction.commit().unwrap();

    let connection = Connection::open(&database).unwrap();
    assert_eq!(note_snapshot(&connection, id).tags, ["holder"]);
    drop(connection);
    run_nt(&home, &["update", id, "tag", "+contender"]);
    let connection = Connection::open(database).unwrap();
    assert_eq!(note_snapshot(&connection, id).tags, ["contender", "holder"]);
    assert_search_index_consistent(&connection);
    assert_foreign_keys(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn direct_mutations_preserve_cross_vault_memberships_and_foreign_keys() {
    let root = temp_dir("direct-fk-integrity");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    run_nt(&home, &["init", "work"]);
    let target = run_nt_with_stdin(&home, &["note", "home:personal/inbox"], "# Link target\n");
    let target_id = target.trim().strip_prefix("saved ").unwrap();
    let note = run_nt_with_stdin(
        &home,
        &[
            "note",
            "home:personal/projects",
            "collection:work/shared",
            &format!("link:{target_id}"),
            "tag:shared",
            "source:https://example.com/shared",
        ],
        "# Cross vault\n",
    );
    let id = note.trim().strip_prefix("saved ").unwrap();
    let database = home.join(".nt/nt.sqlite3");
    assert_foreign_keys(&Connection::open(&database).unwrap());

    run_nt(&home, &["update", id, "home", "work/shared"]);
    run_nt(&home, &["update", id, "collection", "-personal/projects"]);
    let shown = run_nt(&home, &["show", id]);
    assert!(shown.contains("home work/shared"));
    assert!(shown.contains("collections work/shared"));
    assert_foreign_keys(&Connection::open(&database).unwrap());

    run_nt(&home, &["rm", target_id]);
    assert!(run_nt(&home, &["show", id]).contains("links -"));
    assert_foreign_keys(&Connection::open(&database).unwrap());
    run_nt(&home, &["rm", id]);
    assert_foreign_keys(&Connection::open(database).unwrap());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_does_not_materialize_unrequested_bodies_or_relationships() {
    let root = temp_dir("list-projection-pushdown");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let target = run_nt_with_stdin(&home, &["note"], "# Target\n");
    let target_id = target.trim().strip_prefix("saved ").unwrap();
    run_nt_with_stdin(
        &home,
        &[
            "note",
            "tag:project",
            "collection:personal/archive",
            "source:https://example.com/spec",
            &format!("link:{target_id}"),
        ],
        "# Subject\n",
    );

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF")
        .unwrap();
    connection
        .execute("UPDATE notes SET body = x'80'", [])
        .unwrap();
    connection
        .execute("UPDATE note_tags SET tag = x'80'", [])
        .unwrap();
    connection
        .execute("UPDATE note_sources SET source = x'80'", [])
        .unwrap();
    connection
        .execute("UPDATE note_links SET target_id = x'80'", [])
        .unwrap();
    connection
        .execute(
            "UPDATE collections SET name = x'80' WHERE name = 'archive'",
            [],
        )
        .unwrap();
    drop(connection);

    let mut titles = run_nt(&home, &["list", "title"])
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    titles.sort();
    assert_eq!(titles, vec!["Subject", "Target"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn structured_find_does_not_materialize_bodies_or_unqueried_relationships() {
    let root = temp_dir("find-structured-pushdown");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let target = run_nt_with_stdin(&home, &["note"], "# Target\n");
    let target_id = target.trim().strip_prefix("saved ").unwrap();
    let subject = run_nt_with_stdin(
        &home,
        &[
            "note",
            "tag:project",
            "collection:personal/archive",
            "source:https://example.com/spec",
            &format!("link:{target_id}"),
        ],
        "# Subject\n",
    );
    let subject_id = subject.trim().strip_prefix("saved ").unwrap();

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF")
        .unwrap();
    connection
        .execute("UPDATE notes SET body = x'80'", [])
        .unwrap();
    connection
        .execute("UPDATE note_sources SET source = x'80'", [])
        .unwrap();
    connection
        .execute("UPDATE note_links SET target_id = x'80'", [])
        .unwrap();
    connection
        .execute(
            "UPDATE collections SET name = x'80' WHERE name = 'archive'",
            [],
        )
        .unwrap();
    drop(connection);

    let found = run_nt(&home, &["find", "kind:note"]);
    let ids = summary_ids(&found);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&target_id));
    assert!(ids.contains(&subject_id));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fts_find_defines_lexical_semantics_and_preserves_recency_order() {
    let root = temp_dir("find-fts-semantics");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let older = run_nt_with_stdin(
        &home,
        &["note", "tag:first", "tag:shared"],
        "# Storage Decision\n\nThe MicroVM/jailer shares a lexical token.\n",
    );
    let older_id = older.trim().strip_prefix("saved ").unwrap().to_string();
    let newer = run_nt_with_stdin(
        &home,
        &["note", "tag:second", "tag:shared"],
        "# Storage Alternative\n\nA jailer starts another microvm token.\n",
    );
    let newer_id = newer.trim().strip_prefix("saved ").unwrap().to_string();

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute(
            "UPDATE notes SET created = '2026-01-01T00:00:00Z' WHERE id = ?1",
            [&older_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE notes SET created = '2026-01-02T00:00:00Z' WHERE id = ?1",
            [&newer_id],
        )
        .unwrap();
    assert_search_index_consistent(&connection);
    drop(connection);

    for expression in [
        "body:microvm jailer",
        "body:\"jailer microvm\"",
        "body:MICROVM/jailer",
    ] {
        assert_eq!(
            summary_ids(&run_nt(&home, &["find", expression])),
            vec![newer_id.as_str(), older_id.as_str()]
        );
    }
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "title:storage decision"])),
        vec![older_id.as_str()]
    );
    assert!(run_nt(&home, &["find", "title:microvm"]).is_empty());
    assert!(run_nt(&home, &["find", "body:micro"]).is_empty());
    assert!(run_nt(&home, &["find", "body:micro*"]).is_empty());
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "microvm"])),
        vec![newer_id.as_str(), older_id.as_str()]
    );
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "storage jailer"])),
        vec![newer_id.as_str(), older_id.as_str()]
    );
    assert!(run_nt(&home, &["find", "not:body:microvm"]).is_empty());
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "not:body:absent"])),
        vec![newer_id.as_str(), older_id.as_str()]
    );
    assert!(run_nt(&home, &["find", "body:microvm OR absent"]).is_empty());
    assert!(run_nt(&home, &["find", "body:---"]).is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fts_index_tracks_insert_edit_and_delete_transactionally() {
    let root = temp_dir("find-fts-lifecycle");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(
        &home,
        &["note"],
        "# Legacy Heading\n\nAn obsoleteword remains here.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "title:legacy"])),
        vec![id]
    );
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:obsoleteword"])),
        vec![id]
    );
    let database = home.join(".nt/nt.sqlite3");
    let mut connection = Connection::open(&database).unwrap();
    assert_search_index_consistent(&connection);
    let transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "UPDATE notes SET title = 'Rolled Back', body = 'rollbackword' WHERE id = ?1",
            [id],
        )
        .unwrap();
    transaction.rollback().unwrap();
    assert_search_index_consistent(&connection);
    drop(connection);
    assert!(run_nt(&home, &["find", "body:rollbackword"]).is_empty());
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:obsoleteword"])),
        vec![id]
    );

    let editor = root.join("replace-editor.sh");
    fs::write(
        &editor,
        "#!/bin/sh\nprintf '%s\\n' '# Fresh Heading' '' 'A replacementword lives here.' > \"$1\"\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(nt_bin())
        .env("HOME", &home)
        .env("EDITOR", &editor)
        .args(["open", id])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "nt open failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(run_nt(&home, &["find", "title:legacy"]).is_empty());
    assert!(run_nt(&home, &["find", "body:obsoleteword"]).is_empty());
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "title:fresh"])),
        vec![id]
    );
    assert_eq!(
        summary_ids(&run_nt(&home, &["find", "body:replacementword"])),
        vec![id]
    );
    let connection = Connection::open(&database).unwrap();
    assert_search_index_consistent(&connection);
    drop(connection);

    run_nt(&home, &["rm", id]);
    assert!(run_nt(&home, &["find", "replacementword"]).is_empty());
    let connection = Connection::open(database).unwrap();
    assert_search_index_consistent(&connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fts_search_normalizes_unicode_case_and_latin_diacritics() {
    let root = temp_dir("find-fts-unicode");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(
        &home,
        &["note"],
        "# Café Škoda \n\nA NAÏVE café visitor studies 東京 and re\u{301}sume\u{301}.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    for expression in ["title:CAFÉ", "title:škoda", "body:naïve", "body:東京"] {
        assert_eq!(
            summary_ids(&run_nt(&home, &["find", expression])),
            vec![id],
            "Unicode query failed for {expression:?}"
        );
    }
    for expression in [
        "body:cafe",
        "body:naive",
        "body:resume",
        "body:re\u{301}sume\u{301}",
    ] {
        assert_eq!(
            summary_ids(&run_nt(&home, &["find", expression])),
            vec![id],
            "diacritic-insensitive query failed for {expression:?}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fts_results_do_not_materialize_matching_bodies() {
    let root = temp_dir("find-fts-body-projection");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let saved = run_nt_with_stdin(
        &home,
        &["note"],
        "# Indexed body\n\nA uniquestaletoken is indexed.\n",
    );
    let id = saved.trim().strip_prefix("saved ").unwrap();

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute_batch("DROP TRIGGER notes_search_update")
        .unwrap();
    connection
        .execute("UPDATE notes SET body = x'80' WHERE id = ?1", [id])
        .unwrap();
    drop(connection);

    assert_eq!(
        summary_ids(&run_nt(
            &home,
            &["find", "kind:note", "body:uniquestaletoken"]
        )),
        vec![id]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fts_find_combines_structured_and_text_filters() {
    let root = temp_dir("find-mixed-query");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let matching = run_nt_with_stdin(
        &home,
        &["todo", "tag:rust", "source:https://Example.COM/spec"],
        "# Storage Decision\n\nThe JAILER starts a MicroVM.\n",
    );
    let matching_id = matching.trim().strip_prefix("saved ").unwrap();
    run_nt_with_stdin(
        &home,
        &["todo", "tag:rust", "source:https://example.com/spec"],
        "# Storage Alternative\n\nThe jailer starts a process.\n",
    );
    run_nt_with_stdin(
        &home,
        &[
            "todo",
            "status:waiting",
            "tag:rust",
            "source:https://example.com/spec",
        ],
        "# Storage Decision\n\nThe jailer starts a microvm.\n",
    );

    let found = run_nt(
        &home,
        &[
            "find",
            "kind:todo",
            "tag:RUST",
            "not:status:waiting",
            "title:STORAGE",
            "source:EXAMPLE.COM",
            "body:microvm jailer",
            "decision",
        ],
    );
    assert_eq!(summary_ids(&found), vec![matching_id]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_binds_filter_values_as_sql_parameters() {
    let root = temp_dir("find-parameterized-filter");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let tagged = run_nt_with_stdin(&home, &["note"], "# Tagged\n");
    let tagged_id = tagged.trim().strip_prefix("saved ").unwrap();
    run_nt_with_stdin(&home, &["note"], "# Untagged\n");
    let value = "x' OR 1=1 --";
    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute(
            "INSERT INTO note_tags (note_id, tag) VALUES (?1, ?2)",
            [tagged_id, value],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        summary_ids(&run_nt(&home, &["find", &format!("tag:{value}")])),
        vec![tagged_id]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_structured_filters_match_find_semantics() {
    let root = temp_dir("list-filter-parity");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let target = run_nt_with_stdin(&home, &["note"], "# Target\n");
    let target_id = target.trim().strip_prefix("saved ").unwrap().to_string();
    let open = run_nt_with_stdin(
        &home,
        &[
            "todo",
            "priority:A",
            "scheduled:2026-06-01",
            "due:2026-06-02",
            "tag:rust",
            "home:personal/projects",
            "collection:personal/archive",
            &format!("link:{target_id}"),
        ],
        "# Open todo\n",
    );
    let open_id = open.trim().strip_prefix("saved ").unwrap().to_string();
    let waiting = run_nt_with_stdin(
        &home,
        &["todo", "status:waiting", "tag:rust", "tag:draft"],
        "# Waiting todo\n",
    );
    let waiting_id = waiting.trim().strip_prefix("saved ").unwrap().to_string();
    let done = run_nt_with_stdin(
        &home,
        &["todo", "status:done", "tag:shipped"],
        "# Done todo\n",
    );
    let done_id = done.trim().strip_prefix("saved ").unwrap().to_string();

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    for (id, created) in [
        (&target_id, "2026-05-27T10:00:00Z"),
        (&open_id, "2026-05-28T10:00:00Z"),
        (&waiting_id, "2026-05-29T10:00:00Z"),
        (&done_id, "2026-05-30T10:00:00Z"),
    ] {
        connection
            .execute("UPDATE notes SET created = ?1 WHERE id = ?2", [created, id])
            .unwrap();
    }
    connection
        .execute(
            "UPDATE notes SET closed = '2026-05-30T12:00:00Z' WHERE id = ?1",
            [&done_id],
        )
        .unwrap();
    drop(connection);

    let cases = vec![
        vec![format!("id:{}", &open_id[..8])],
        vec!["#rust".to_string()],
        vec!["tag:rust".to_string()],
        vec!["day:2026-05-28".to_string()],
        vec!["since:2026-05-29".to_string()],
        vec!["before:2026-05-29".to_string()],
        vec!["kind:todo".to_string()],
        vec!["status:open".to_string()],
        vec!["priority:a".to_string()],
        vec!["scheduled:2026-06-01".to_string()],
        vec!["due:2026-06-02".to_string()],
        vec!["closed:2026-05-30".to_string()],
        vec!["collection:personal/archive".to_string()],
        vec![format!("link:{target_id}")],
        vec!["not:tag:draft".to_string()],
        vec!["not:status:waiting".to_string()],
        vec![
            "kind:todo".to_string(),
            "tag:rust".to_string(),
            "not:tag:draft".to_string(),
            "since:2026-05-01".to_string(),
            "before:2026-06-01".to_string(),
        ],
    ];

    for filters in cases {
        let mut list_args = vec!["list", "id"];
        list_args.extend(filters.iter().map(String::as_str));
        let mut find_args = vec!["find"];
        find_args.extend(filters.iter().map(String::as_str));
        let listed = run_nt(&home, &list_args);
        let found = run_nt(&home, &find_args);
        assert_eq!(
            summary_ids(&listed),
            summary_ids(&found),
            "list/find mismatch for {filters:?}"
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_projects_multiple_relationship_sets_as_one_row() {
    let root = temp_dir("list-set-projections");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let first = run_nt_with_stdin(&home, &["note"], "# First target\n");
    let first_id = first.trim().strip_prefix("saved ").unwrap().to_string();
    let second = run_nt_with_stdin(&home, &["note"], "# Second target\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap().to_string();
    let subject = run_nt_with_stdin(
        &home,
        &[
            "note",
            "home:personal/inbox",
            "tag:pi",
            "tag:alpha",
            "collection:personal/pi",
            "collection:personal/alpha",
            &format!("link:{second_id}"),
            &format!("link:{first_id}"),
            "source:https://p.example",
            "source:https://a.example",
        ],
        "# Subject\n",
    );
    let subject_id = subject.trim().strip_prefix("saved ").unwrap();

    let output = run_nt(
        &home,
        &[
            "list",
            "id,tag,collection,link,source",
            &format!("id:{subject_id}"),
        ],
    );
    let rows = output.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    let values = rows[0].split('\t').collect::<Vec<_>>();
    let mut links = [first_id, second_id];
    links.sort();
    assert_eq!(values[0], subject_id);
    assert_eq!(values[1], "alpha,pi");
    assert_eq!(values[2], "personal/alpha,personal/inbox,personal/pi");
    assert_eq!(values[3], links.join(","));
    assert_eq!(values[4], "https://a.example,https://p.example");

    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute("UPDATE notes SET body = x'80' WHERE id = ?1", [subject_id])
        .unwrap();
    drop(connection);
    let all = run_nt(&home, &["list", "all", &format!("id:{subject_id}")]);
    assert_eq!(all.lines().count(), 1);
    assert_eq!(all.trim_end().split('\t').count(), 15);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_binds_filter_values_as_sql_parameters() {
    let root = temp_dir("list-parameterized-filter");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let tagged = run_nt_with_stdin(&home, &["note"], "# Tagged\n");
    let tagged_id = tagged.trim().strip_prefix("saved ").unwrap();
    run_nt_with_stdin(&home, &["note"], "# Untagged\n");
    let value = "x' OR 1=1 --";
    let connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    connection
        .execute(
            "INSERT INTO note_tags (note_id, tag) VALUES (?1, ?2)",
            [tagged_id, value],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        run_nt(&home, &["list", "id", &format!("tag:{value}")]),
        format!("{tagged_id}\n")
    );
    let _ = fs::remove_dir_all(root);
}

#[derive(Debug, PartialEq)]
struct NoteSnapshot {
    row: StoredNoteRow,
    collections: Vec<String>,
    tags: Vec<String>,
    sources: Vec<String>,
    links: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct StoredNoteRow {
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

fn note_snapshot(connection: &Connection, id: &str) -> NoteSnapshot {
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

fn install_note_audit(connection: &Connection, id: &str) {
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

fn audit_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM mutation_audit", [], |row| row.get(0))
        .unwrap()
}

fn assert_foreign_keys(connection: &Connection) {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

fn assert_search_index_consistent(connection: &Connection) {
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
