mod common;

use std::fs;

use common::{
    assert_failed, assert_foreign_keys, assert_search_index_consistent, audit_count,
    install_note_audit, note_snapshot, run_nt, run_nt_with_stdin, temp_dir,
};
use rusqlite::Connection;
use uuid::Uuid;

#[test]
fn remove_is_transactional_and_cleans_relationships() {
    let root = temp_dir("remove-relations");
    let home = root.join("home");
    run_nt(&home, &["init", "personal"]);
    let second = run_nt_with_stdin(&home, &["note"], "# Second\n");
    let second_id = second.trim().strip_prefix("saved ").unwrap();
    let first = run_nt_with_stdin(
        &home,
        &[
            "note",
            "home:personal/inbox",
            "tag:cleanup",
            "source:https://example.com/cleanup",
            "collection:personal/archive",
            &format!("link:{second_id}"),
        ],
        "# First\n\nSearchable cleanup token.\n",
    );
    let first_id = first.trim().strip_prefix("saved ").unwrap();
    run_nt(
        &home,
        &["update", second_id, "link", &format!("+{first_id}")],
    );

    let database = home.join(".nt/nt.sqlite3");
    let connection = Connection::open(&database).unwrap();
    let search_id: i64 = connection
        .query_row(
            "SELECT search_id FROM note_search_rows WHERE note_id = ?1",
            [first_id],
            |row| row.get(0),
        )
        .unwrap();
    for (table, expected) in [
        ("note_tags", 1),
        ("note_sources", 1),
        ("note_collections", 2),
    ] {
        let count: i64 = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE note_id = ?1"),
                [first_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, expected, "missing {table} cleanup fixture");
    }
    let link_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM note_links WHERE note_id = ?1 OR target_id = ?1",
            [first_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(link_count, 2);
    assert_search_index_consistent(&connection);
    drop(connection);

    run_nt(&home, &["rm", first_id]);
    let shown = run_nt(&home, &["show", second_id]);
    assert!(shown.contains("links -"));

    let connection = Connection::open(database).unwrap();
    let note_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(note_count, 1);
    for table in [
        "note_tags",
        "note_sources",
        "note_collections",
        "note_search_rows",
    ] {
        let count: i64 = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE note_id = ?1"),
                [first_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "{table} row was not cleaned up");
    }
    let link_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM note_links WHERE note_id = ?1 OR target_id = ?1",
            [first_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(link_count, 0);
    let fts_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM note_fts WHERE rowid = ?1",
            [search_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts_count, 0);
    assert_search_index_consistent(&connection);
    assert_foreign_keys(&connection);

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
