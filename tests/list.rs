mod common;

use std::fs;

use common::{run_nt, run_nt_with_stdin, summary_ids, temp_dir};
use rusqlite::Connection;

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
