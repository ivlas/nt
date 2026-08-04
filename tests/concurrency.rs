mod common;

use std::fs;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use common::{
    assert_foreign_keys, assert_search_index_consistent, note_snapshot, nt_bin, run_nt,
    run_nt_with_stdin, summary_ids, temp_dir,
};
use rusqlite::{Connection, TransactionBehavior};

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
