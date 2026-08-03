mod common;

use std::fs;

use common::{assert_failed, run_nt, run_nt_with_stdin, temp_dir};
use rusqlite::Connection;

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
