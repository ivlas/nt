use std::path::Path;
use std::process::{Command, Output, Stdio};

use rusqlite::{Connection, params};

#[test]
fn help_output_matches_goldens() {
    let home = tempfile::tempdir().unwrap();

    assert_stdout(
        run(home.path(), &["help"]),
        include_bytes!("fixtures/root-help.txt"),
    );
    assert_stdout(
        run(home.path(), &["help", "memory"]),
        include_bytes!("fixtures/memory-help.txt"),
    );
}

#[test]
fn memory_pending_expand_and_status_match_goldens() {
    let home = tempfile::tempdir().unwrap();
    assert_stdout(run(home.path(), &["init"]), b"initialized\n");
    seed_memory(home.path());

    assert_stdout(
        run(home.path(), &["memory", "pending", "L0:0"]),
        include_bytes!("fixtures/memory-pending-prompt.txt"),
    );
    assert_stdout(
        run(home.path(), &["memory", "expand", "L0:0"]),
        include_bytes!("fixtures/memory-expand.txt"),
    );
    assert_stdout(
        run(home.path(), &["memory", "status"]),
        include_bytes!("fixtures/memory-status.txt"),
    );
}

fn seed_memory(home: &Path) {
    let mut connection = Connection::open(home.join(".nt/nt.sqlite3")).unwrap();
    let transaction = connection.transaction().unwrap();
    for seq in 1..=16 {
        transaction
            .execute(
                "INSERT INTO memories(seq, body, created) VALUES (?1, ?2, ?3)",
                params![seq, format!("event {seq}"), "2026-08-22T12:34:56Z"],
            )
            .unwrap();
    }
    transaction
        .execute(
            "INSERT INTO memory_segments(level, block, summary, created)
             VALUES (0, 0, 'events one through sixteen', '2026-08-22T12:34:56Z')",
            [],
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO memory_summary_jobs(level, block) VALUES (0, 0)",
            [],
        )
        .unwrap();
    transaction.commit().unwrap();
}

fn run(home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn assert_stdout(output: Output, expected: &[u8]) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}
