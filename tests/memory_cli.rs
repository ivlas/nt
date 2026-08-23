use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use rusqlite::Connection;

#[test]
fn memory_cli_preserves_exact_history_and_supports_recall_and_filters() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    assert_success(
        nt(
            home.path(),
            &[
                "memory",
                "add",
                "--",
                "Deployment failed because port 8080 was occupied.",
            ],
            None,
        ),
        b"saved 1\n",
    );
    assert_success(
        nt(
            home.path(),
            &["memory", "add"],
            Some(b"We changed deployment strategy to blue-green.\r\n"),
        ),
        b"saved 2\n",
    );

    assert_success(
        nt(home.path(), &["memory", "show", "2"], None),
        b"We changed deployment strategy to blue-green.\n",
    );
    let listed = nt(
        home.path(),
        &["memory", "list", "since:2", "until:2", "limit:1"],
        None,
    );
    assert!(listed.status.success(), "{:?}", listed.stderr);
    let columns = String::from_utf8(listed.stdout)
        .unwrap()
        .trim_end()
        .split('\t')
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0], "2");
    assert_eq!(
        serde_json::from_str::<String>(&columns[2]).unwrap(),
        "We changed deployment strategy to blue-green.\n"
    );

    let recalled = nt(
        home.path(),
        &["memory", "recall", "deployment", "strategy", "limit:1"],
        None,
    );
    assert!(recalled.status.success(), "{:?}", recalled.stderr);
    assert!(
        String::from_utf8(recalled.stdout)
            .unwrap()
            .starts_with("2\t")
    );

    let missing = nt(home.path(), &["memory", "show", "99"], None);
    assert_eq!(missing.status.code(), Some(3));
    assert_eq!(missing.stderr, b"error: memory not found: 99\n");
}

#[test]
fn memory_limits_count_unicode_characters_and_reject_nul() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");

    let accepted = "é".repeat(1_024);
    assert_success(
        nt(home.path(), &["memory", "add"], Some(accepted.as_bytes())),
        b"saved 1\n",
    );
    let rejected = "é".repeat(1_025);
    let output = nt(home.path(), &["memory", "add"], Some(rejected.as_bytes()));
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("exceeds 1024 characters")
    );

    let output = nt(home.path(), &["memory", "add"], Some(b"a\0b"));
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("contains NUL")
    );
}

#[test]
fn summary_workflow_is_explicit_expandable_and_invalidatable() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    for seq in 1..=16 {
        let body = format!("event {seq}");
        assert_success(
            nt(home.path(), &["memory", "add"], Some(body.as_bytes())),
            format!("saved {seq}\n").as_bytes(),
        );
    }

    let pending = nt(home.path(), &["memory", "pending"], None);
    assert_success(pending, b"L0:0\t1-16\t0\n");
    let task = nt(home.path(), &["memory", "pending", "L0:0"], None);
    assert!(task.status.success(), "{:?}", task.stderr);
    let task = String::from_utf8(task.stdout).unwrap();
    assert!(task.contains("Compress these children into one factual summary."));
    assert!(task.contains("Maximum: 1024 characters."));
    assert_eq!(
        task.lines()
            .filter(|line| line.starts_with(|ch: char| ch.is_ascii_digit()))
            .count(),
        16
    );

    assert_success(
        nt(
            home.path(),
            &[
                "memory",
                "summarize",
                "L0:0",
                "--",
                "events",
                "one",
                "through",
                "sixteen",
            ],
            None,
        ),
        b"summarized L0:0\n",
    );
    assert_success(
        nt(
            home.path(),
            &[
                "memory",
                "summarize",
                "L0:0",
                "--",
                "events",
                "one",
                "through",
                "sixteen",
            ],
            None,
        ),
        b"summarized L0:0\n",
    );
    assert_success(
        nt(home.path(), &["memory", "show", "L0:0"], None),
        b"events one through sixteen",
    );
    assert_success(nt(home.path(), &["memory", "show", "1"], None), b"event 1");
    let conflict = nt(
        home.path(),
        &["memory", "summarize", "L0:0", "--", "different"],
        None,
    );
    assert_eq!(conflict.status.code(), Some(2));
    assert!(
        String::from_utf8(conflict.stderr)
            .unwrap()
            .contains("conflicts with existing summary")
    );

    let expanded = nt(home.path(), &["memory", "expand", "L0:0"], None);
    assert!(expanded.status.success(), "{:?}", expanded.stderr);
    let expanded = String::from_utf8(expanded.stdout).unwrap();
    assert_eq!(expanded.lines().count(), 16);
    assert!(!expanded.contains("events one through sixteen"));
    assert!(!expanded.contains("L0:0"));
    let context = nt(home.path(), &["memory", "context", "events"], None);
    assert!(context.status.success(), "{:?}", context.stderr);
    assert!(
        String::from_utf8(context.stdout)
            .unwrap()
            .contains("event 16")
    );

    assert_success(
        nt(home.path(), &["memory", "invalidate", "L0:0"], None),
        b"invalidated L0:0\n",
    );
    assert_success(nt(home.path(), &["memory", "show", "1"], None), b"event 1");
    assert_success(
        nt(home.path(), &["memory", "pending"], None),
        b"L0:0\t1-16\t0\n",
    );
}

#[test]
fn memory_show_rejects_missing_summaries_and_invalid_targets() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");

    let missing = nt(home.path(), &["memory", "show", "L0:99"], None);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(
        missing.stderr,
        b"error: invalid memory node: L0:99 summary not found\n"
    );

    for target in ["0", "Lx:0", "L0:-1"] {
        let invalid = nt(home.path(), &["memory", "show", target], None);
        assert_eq!(invalid.status.code(), Some(2), "{target}");
        assert!(invalid.stdout.is_empty(), "{target}");
        assert!(!invalid.stderr.is_empty(), "{target}");
    }
}

#[test]
fn higher_level_expand_returns_only_direct_child_summaries() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    let mut connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    let transaction = connection.transaction().unwrap();
    for seq in 1..=256 {
        transaction
            .execute(
                "INSERT INTO memories(seq, body, created) VALUES (?1, ?2, ?3)",
                rusqlite::params![seq, format!("event {seq}"), "2026-08-22T12:34:56Z"],
            )
            .unwrap();
    }
    for block in 0..16 {
        transaction
            .execute(
                "INSERT INTO memory_summary_jobs(level, block) VALUES (0, ?1)",
                [block],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);

    for block in 0..16 {
        let node = format!("L0:{block}");
        let summary = format!("child summary {block}");
        assert_success(
            nt(
                home.path(),
                &["memory", "summarize", &node],
                Some(summary.as_bytes()),
            ),
            format!("summarized {node}\n").as_bytes(),
        );
    }
    assert_success(
        nt(
            home.path(),
            &["memory", "summarize", "L1:0"],
            Some(b"level one parent"),
        ),
        b"summarized L1:0\n",
    );

    let expanded = nt(home.path(), &["memory", "expand", "L1:0"], None);
    assert!(expanded.status.success(), "{:?}", expanded.stderr);
    let expanded = String::from_utf8(expanded.stdout).unwrap();
    let lines = expanded.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 16);
    for (block, line) in lines.iter().enumerate() {
        assert!(line.starts_with(&format!("L0:{block}\t")), "{line}");
    }
    assert!(!expanded.contains("L1:0"));
    assert!(!expanded.contains("level one parent"));
}

#[test]
fn concurrent_process_appends_get_distinct_monotonic_sequences() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    let home_path = home.path();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for index in 0..24 {
            handles.push(scope.spawn(move || {
                let body = format!("concurrent {index}");
                nt(home_path, &["memory", "add"], Some(body.as_bytes()))
            }));
        }
        for handle in handles {
            let output = handle.join().unwrap();
            assert!(output.status.success(), "{:?}", output.stderr);
        }
    });

    let output = nt(home.path(), &["memory", "list"], None);
    assert!(output.status.success(), "{:?}", output.stderr);
    let sequences = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| line.split('\t').next().unwrap().parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(sequences, (1..=24).collect::<Vec<_>>());
    assert_success(
        nt(home.path(), &["memory", "pending"], None),
        b"L0:0\t1-16\t0\n",
    );
    let recalled = nt(home.path(), &["memory", "recall", "concurrent"], None);
    assert!(recalled.status.success(), "{:?}", recalled.stderr);
    assert_eq!(
        String::from_utf8(recalled.stdout).unwrap().lines().count(),
        24
    );
}

#[test]
fn schema_prevents_raw_memory_update_and_deletion() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    assert_success(
        nt(home.path(), &["memory", "add"], Some(b"immutable")),
        b"saved 1\n",
    );
    let connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    assert!(
        connection
            .execute("UPDATE memories SET body = 'changed' WHERE seq = 1", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM memories WHERE seq = 1", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT OR REPLACE INTO memories(seq, body, created)
                 VALUES (1, 'replacement', '2026-08-22T12:34:56Z')",
                [],
            )
            .is_err()
    );
    let body: String = connection
        .query_row("SELECT body FROM memories WHERE seq = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(body, "immutable");
    let original_fts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'immutable'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let replacement_fts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'replacement'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((original_fts, replacement_fts), (1, 0));
}

#[test]
fn memory_retrieval_commands_use_read_only_connections() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    for seq in 1..=16 {
        let body = format!("readonly memory {seq}");
        assert_success(
            nt(home.path(), &["memory", "add"], Some(body.as_bytes())),
            format!("saved {seq}\n").as_bytes(),
        );
    }
    assert_success(
        nt(
            home.path(),
            &["memory", "summarize", "L0:0"],
            Some(b"readonly summary"),
        ),
        b"summarized L0:0\n",
    );

    let database = home.path().join(".nt/nt.sqlite3");
    let original = std::fs::metadata(&database).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&database, readonly).unwrap();

    for arguments in [
        vec!["memory", "show", "1"],
        vec!["memory", "show", "L0:0"],
        vec!["memory", "list", "limit:1"],
        vec!["memory", "recall", "readonly", "limit:1"],
        vec!["memory", "context", "readonly"],
        vec!["memory", "pending"],
        vec!["memory", "expand", "L0:0"],
        vec!["memory", "status"],
    ] {
        let output = nt(home.path(), &arguments, None);
        assert!(
            output.status.success(),
            "{arguments:?}: {:?}",
            output.stderr
        );
    }

    std::fs::set_permissions(database, original).unwrap();
}

#[test]
fn context_stdout_including_metadata_stays_within_the_character_limit() {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    let database = home.path().join(".nt/nt.sqlite3");
    let mut connection = Connection::open(database).unwrap();
    let body = format!("needle{}", "é".repeat(1_018));
    let transaction = connection.transaction().unwrap();
    for _ in 0..40 {
        transaction
            .execute(
                "INSERT INTO memories(body, created) VALUES (?1, '2026-08-22T12:34:56Z')",
                [&body],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);

    let output = nt(home.path(), &["memory", "context", "needle"], None);
    assert!(output.status.success(), "{:?}", output.stderr);
    let context = String::from_utf8(output.stdout).unwrap();
    assert!(context.chars().count() <= 32_768);
    assert!(context.contains("# memory "));
    assert!(context.contains("2026-08-22T12:34:56Z"));
    assert!(context.contains(&body));
}

fn nt(home: &Path, arguments: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nt"));
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().unwrap();
    if let Some(stdin) = stdin {
        child.stdin.take().unwrap().write_all(stdin).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn assert_success(output: Output, expected_stdout: &[u8]) {
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.is_empty());
}
