mod common;

use std::fs;

use common::{assert_search_index_consistent, run_nt, run_nt_with_stdin, summary_ids, temp_dir};
use rusqlite::Connection;

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
fn fts_index_tracks_insert_update_and_delete_transactionally() {
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

    let connection = Connection::open(&database).unwrap();
    connection
        .execute(
            "UPDATE notes SET title = 'Fresh Heading', body = 'A replacementword lives here.' WHERE id = ?1",
            [id],
        )
        .unwrap();
    drop(connection);

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
    let update_trigger_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = 'notes_search_update'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute_batch("DROP TRIGGER notes_search_update")
        .unwrap();
    connection
        .execute("UPDATE notes SET body = x'80' WHERE id = ?1", [id])
        .unwrap();
    connection.execute_batch(&update_trigger_sql).unwrap();
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
