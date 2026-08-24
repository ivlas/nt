use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use rusqlite::{Connection, params};

#[test]
fn append_wake_and_literal_recall_use_raw_chronological_history() {
    let home = initialized_home();
    assert_success(
        nt(
            home.path(),
            &["memory", "add", "Port 8080 was occupied."],
            None,
        ),
        b"saved #0\n",
    );
    assert_success(
        nt(
            home.path(),
            &["memory", "add"],
            Some(b"Changed deployment strategy.\n"),
        ),
        b"saved #1\n",
    );

    assert_success(
        nt(home.path(), &["memory", "wake"], None),
        b"#0 Port 8080 was occupied.\n#1 Changed deployment strategy.\n",
    );
    assert_success(
        nt(home.path(), &["memory", "recall", "deployment"], None),
        b"#1 Changed deployment strategy.\n",
    );
    assert_success(
        nt(home.path(), &["memory", "recall", "Deployment"], None),
        b"",
    );
}

#[test]
fn memory_validation_enforces_concise_single_lines() {
    let home = initialized_home();
    assert_success(
        nt(
            home.path(),
            &["memory", "add"],
            Some("é".repeat(512).as_bytes()),
        ),
        b"saved #0\n",
    );
    for (body, message) in [
        ("é".repeat(513), "exceeds 512 characters"),
        ("first\nsecond".to_string(), "must be one line"),
        ("nul\0byte".to_string(), "contains NUL"),
    ] {
        let output = nt(home.path(), &["memory", "add"], Some(body.as_bytes()));
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
}

#[test]
fn nap_builds_binary_summaries_and_zoom_reveals_direct_children() {
    let home = initialized_home();
    append_events(home.path(), 8);

    let task = nt(home.path(), &["memory", "nap"], None);
    assert!(task.status.success());
    assert_eq!(
        String::from_utf8(task.stdout).unwrap(),
        "Compress memories #0-1 into one short memory:\n\n#0 event 0\n#1 event 1\n\nRun:\nnt memory nap 0-1 \"<summary>\"\n"
    );

    for (range, body) in [
        ("0-1", "events zero and one"),
        ("2-3", "events two and three"),
        ("4-5", "events four and five"),
        ("6-7", "events six and seven"),
        ("0-3", "events zero through three"),
        ("4-7", "events four through seven"),
        ("0-7", "events zero through seven"),
    ] {
        assert_success(
            nt(home.path(), &["memory", "nap", range, body], None),
            format!("summarized #{range}\n").as_bytes(),
        );
    }
    assert_success(
        nt(home.path(), &["memory", "nap"], None),
        b"nothing to nap\n",
    );
    assert_success(
        nt(home.path(), &["memory", "zoom", "0-7"], None),
        b"#0-3 events zero through three\n#4-7 events four through seven\n",
    );
    assert_success(
        nt(home.path(), &["memory", "zoom", "0-1"], None),
        b"#0 event 0\n#1 event 1\n",
    );
}

#[test]
fn forget_removes_dependent_ancestors_and_preserves_raw_history() {
    let home = initialized_home();
    append_events(home.path(), 4);
    for (range, body) in [
        ("0-1", "first pair"),
        ("2-3", "second pair"),
        ("0-3", "all four"),
    ] {
        assert_success(
            nt(home.path(), &["memory", "nap", range, body], None),
            format!("summarized #{range}\n").as_bytes(),
        );
    }

    assert_success(
        nt(home.path(), &["memory", "forget", "0-1"], None),
        b"forgot #0-1\n",
    );
    assert_success(
        nt(home.path(), &["memory", "wake"], None),
        b"#0 event 0\n#1 event 1\n#2 event 2\n#3 event 3\n",
    );
    let task = nt(home.path(), &["memory", "nap"], None);
    assert!(
        String::from_utf8(task.stdout)
            .unwrap()
            .starts_with("Compress memories #0-1")
    );
    let missing = nt(home.path(), &["memory", "zoom", "0-3"], None);
    assert_eq!(missing.status.code(), Some(2));
}

#[test]
fn wake_is_bounded_deterministic_and_age_decaying_for_large_history() {
    let home = initialized_home();
    let database = home.path().join(".nt/nt.sqlite3");
    let mut connection = Connection::open(database).unwrap();
    let transaction = connection.transaction().unwrap();
    for sequence in 0..1_000_i64 {
        transaction
            .execute(
                "INSERT INTO memory(sequence, created_at, body) VALUES (?1, ?2, ?3)",
                params![
                    sequence,
                    "2026-08-22T12:34:56Z",
                    format!("event {sequence}")
                ],
            )
            .unwrap();
    }
    let mut size = 2_i64;
    while size <= 1_000 {
        let mut lo = 0_i64;
        while lo + size <= 1_000 {
            transaction
                .execute(
                    "INSERT INTO memory_summary(lo, hi, body) VALUES (?1, ?2, ?3)",
                    params![lo, lo + size, format!("events {lo}-{}", lo + size - 1)],
                )
                .unwrap();
            lo += size;
        }
        size *= 2;
    }
    transaction.commit().unwrap();

    let first = nt(home.path(), &["memory", "wake"], None);
    let second = nt(home.path(), &["memory", "wake"], None);
    assert!(first.status.success(), "{:?}", first.stderr);
    assert_eq!(first.stdout, second.stdout);
    let output = String::from_utf8(first.stdout).unwrap();
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 128);
    let sizes = lines
        .iter()
        .map(|line| rendered_size(line))
        .collect::<Vec<_>>();
    assert!(sizes.windows(2).all(|pair| pair[0] >= pair[1]));
    assert!(sizes.first().unwrap() > sizes.last().unwrap());
}

#[test]
fn wake_reports_missing_derived_summaries_without_falling_back_to_search() {
    let home = initialized_home();
    let connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    for sequence in 0..129_i64 {
        connection
            .execute(
                "INSERT INTO memory(sequence, created_at, body) VALUES (?1, ?2, ?3)",
                params![
                    sequence,
                    "2026-08-22T12:34:56Z",
                    format!("event {sequence}")
                ],
            )
            .unwrap();
    }

    let output = nt(home.path(), &["memory", "wake"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("summary missing; run nt memory nap")
    );
}

#[test]
fn concurrent_appends_receive_distinct_zero_based_sequences() {
    let home = initialized_home();
    let home_path = home.path();
    std::thread::scope(|scope| {
        let handles = (0..24)
            .map(|index| {
                scope.spawn(move || {
                    let body = format!("concurrent event {index}");
                    nt(home_path, &["memory", "add", &body], None)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert!(handle.join().unwrap().status.success());
        }
    });
    let output = nt(home.path(), &["memory", "recall", "concurrent event"], None);
    assert!(output.status.success());
    let mut sequences = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            line.split_once(' ')
                .unwrap()
                .0
                .trim_start_matches('#')
                .parse::<u64>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    sequences.sort_unstable();
    assert_eq!(sequences, (0..24).collect::<Vec<_>>());
}

#[test]
fn schema_keeps_raw_memory_immutable_without_memory_fts_or_jobs() {
    let home = initialized_home();
    assert_success(
        nt(home.path(), &["memory", "add", "immutable"], None),
        b"saved #0\n",
    );
    let connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    assert!(
        connection
            .execute("UPDATE memory SET body = 'changed' WHERE sequence = 0", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM memory WHERE sequence = 0", [])
            .is_err()
    );
    let memory_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name LIKE 'memory%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(memory_tables, 2);
}

#[test]
#[ignore = "manual 10k/100k/1m/10m memory operation audit"]
fn audit_memory_operations_at_scale() {
    for count in [10_000_i64, 100_000, 1_000_000, 10_000_000] {
        let home = initialized_home();
        let database = home.path().join(".nt/nt.sqlite3");
        let mut connection = Connection::open(&database).unwrap();
        let setup_started = Instant::now();
        let transaction = connection.transaction().unwrap();
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO memory(sequence, created_at, body)
                     VALUES (?1, '2026-08-22T12:34:56Z', ?2)",
                )
                .unwrap();
            for sequence in 0..count {
                insert
                    .execute(params![sequence, format!("event {sequence}")])
                    .unwrap();
            }
        }
        {
            let mut insert = transaction
                .prepare("INSERT INTO memory_summary(lo, hi, body) VALUES (?1, ?2, 'summary')")
                .unwrap();
            let mut size = 2_i64;
            while size <= count {
                let mut lo = 0_i64;
                while lo + size <= count {
                    insert.execute([lo, lo + size]).unwrap();
                    lo += size;
                }
                size *= 2;
            }
        }
        transaction.commit().unwrap();
        let setup_elapsed = setup_started.elapsed();

        let started = Instant::now();
        let wake = nt(home.path(), &["memory", "wake"], None);
        let wake_elapsed = started.elapsed();
        assert!(wake.status.success(), "{:?}", wake.stderr);

        let started = Instant::now();
        let recall = nt(
            home.path(),
            &["memory", "recall", "definitely-not-present"],
            None,
        );
        let recall_elapsed = started.elapsed();
        assert_success(recall, b"");

        let started = Instant::now();
        let nap = nt(home.path(), &["memory", "nap"], None);
        let nap_elapsed = started.elapsed();
        assert_success(nap, b"nothing to nap\n");

        connection
            .execute("DELETE FROM memory_summary WHERE lo = 0 AND hi = 2", [])
            .unwrap();
        let started = Instant::now();
        let insertion = nt(
            home.path(),
            &["memory", "nap", "0-1", "replacement summary"],
            None,
        );
        let insertion_elapsed = started.elapsed();
        assert_success(insertion, b"summarized #0-1\n");

        eprintln!(
            "{count:>7}: setup={:?} wake={wake_elapsed:?} recall={recall_elapsed:?} nap-next={nap_elapsed:?} insert={insertion_elapsed:?}",
            setup_elapsed
        );
    }
}

fn rendered_size(line: &str) -> u64 {
    let identity = line.split_once(' ').unwrap().0.trim_start_matches('#');
    identity.split_once('-').map_or(1, |(lo, hi)| {
        hi.parse::<u64>().unwrap() - lo.parse::<u64>().unwrap() + 1
    })
}

fn initialized_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    assert_success(nt(home.path(), &["init"], None), b"initialized\n");
    home
}

fn append_events(home: &Path, count: u64) {
    for sequence in 0..count {
        assert_success(
            nt(home, &["memory", "add", &format!("event {sequence}")], None),
            format!("saved #{sequence}\n").as_bytes(),
        );
    }
}

fn nt(home: &Path, arguments: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nt"));
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn().unwrap();
    if let Some(stdin) = stdin {
        child.stdin.take().unwrap().write_all(stdin).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn assert_success(output: Output, expected_stdout: &[u8]) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.is_empty());
}
