mod common;

use std::fs;

use common::{assert_uuid_v7, run_nt, run_nt_with_stdin, summary_ids, temp_dir};
use rusqlite::Connection;

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
            "SELECT n.body, n.title, v.name || '/' || c.name FROM notes n JOIN collections c ON c.id = n.home_collection_id JOIN vaults v ON v.id = c.vault_id WHERE n.id = ?1",
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
