use std::fs;
use std::hint::black_box;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rusqlite::{Connection, params};

const CHECKPOINTS: [usize; 3] = [1_000, 100_000, 1_000_000];
const BATCH_SIZES: [usize; 4] = [32, 64, 96, 128];
const SAMPLES: usize = 5;

#[derive(Clone)]
struct Invocation {
    arguments: Vec<String>,
    stdin: Vec<u8>,
}

impl Invocation {
    fn new(arguments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            arguments: arguments.into_iter().map(Into::into).collect(),
            stdin: Vec::new(),
        }
    }

    fn with_stdin(mut self, stdin: impl Into<Vec<u8>>) -> Self {
        self.stdin = stdin.into();
        self
    }
}

#[derive(Debug)]
struct Measurement {
    database_notes: usize,
    operation: String,
    requested: usize,
    sqlite: Option<Duration>,
    first_row: Duration,
    total: Duration,
    peak_rss_bytes: u64,
}

#[test]
#[ignore = "manual process-level benchmark; creates a one-million-note database"]
fn benchmark_generic_agentic_primitives() {
    let home = tempfile::tempdir().unwrap();
    run(home.path(), &Invocation::new(["init"]));
    let database = home.path().join(".nt/nt.sqlite3");
    let mut connection = Connection::open(&database).unwrap();

    println!("sqlite={}", rusqlite::version());
    println!("samples={SAMPLES}");
    println!("sample_body_bytes={}", fixture_body(0).len());
    let startup = median_duration(
        (0..SAMPLES)
            .map(|_| elapsed(home.path(), &Invocation::new(["help"])))
            .collect(),
    );
    println!("startup_help_ms={:.3}", milliseconds(startup));

    let mut measurements = Vec::new();
    let mut populated = 0;
    for checkpoint in CHECKPOINTS {
        let global_revision = populate(&mut connection, populated, checkpoint);
        populated = checkpoint;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        let database_notes = note_count(&connection);
        assert!(database_notes >= checkpoint);

        let collection = Invocation::new(["read", "collection:bench/target", "limit:100"]);
        measurements.push(measure_read(
            home.path(),
            &connection,
            database_notes,
            "read_collection_limit_100",
            100,
            &collection,
            || sqlite_collection_read(&connection),
        ));

        for batch_size in BATCH_SIZES {
            let ids = arbitrary_ids(checkpoint, batch_size);
            let input = ids.join("\n") + "\n";
            let invocation = Invocation::new(["read", "id:-"]).with_stdin(input);
            measurements.push(measure_read(
                home.path(),
                &connection,
                database_notes,
                "read_ids",
                batch_size,
                &invocation,
                || sqlite_id_read(&connection, &ids),
            ));
        }

        let recent_revision = usize::try_from(global_revision).unwrap().saturating_sub(10);
        let changes = Invocation::new(["changes", &format!("since:{recent_revision}")]);
        measurements.push(measure_read(
            home.path(),
            &connection,
            database_notes,
            "changes_recent",
            10,
            &changes,
            || sqlite_recent_changes(&connection, recent_revision),
        ));

        let add = Invocation::new(["add", "collection:bench/mutations", "--", "# Added", "body"]);
        measurements.push(measure_mutation(
            home.path(),
            database_notes,
            "single_add",
            1,
            |_| add.clone(),
        ));

        let edited_id = fixture_id(checkpoint / 2);
        measurements.push(measure_mutation(
            home.path(),
            database_notes,
            "single_edit",
            1,
            |sample| {
                Invocation::new([
                    "edit".to_string(),
                    edited_id.clone(),
                    "--".to_string(),
                    format!("# Edited {sample}"),
                    "body".to_string(),
                ])
            },
        ));

        let batch_ids = arbitrary_ids(checkpoint, 128);
        let batch_input = batch_ids.join("\n") + "\n";
        measurements.push(measure_mutation(
            home.path(),
            database_notes,
            "batch_move",
            batch_ids.len(),
            |sample| {
                Invocation::new([
                    "move",
                    "id:-",
                    if sample % 2 == 0 {
                        "bench/batch_a"
                    } else {
                        "bench/batch_b"
                    },
                ])
                .with_stdin(batch_input.clone())
            },
        ));
    }

    println!("database_notes,operation,requested,sqlite_ms,first_row_ms,total_ms,peak_rss_mib");
    for measurement in measurements {
        let sqlite = measurement
            .sqlite
            .map(|value| format!("{:.3}", milliseconds(value)))
            .unwrap_or_default();
        println!(
            "{},{},{},{},{:.3},{:.3},{:.2}",
            measurement.database_notes,
            measurement.operation,
            measurement.requested,
            sqlite,
            milliseconds(measurement.first_row),
            milliseconds(measurement.total),
            measurement.peak_rss_bytes as f64 / 1_048_576.0,
        );
    }
    println!("database_bytes={}", database_bytes(&database));
}

fn measure_read(
    home: &Path,
    connection: &Connection,
    database_notes: usize,
    operation: &str,
    requested: usize,
    invocation: &Invocation,
    sqlite_operation: impl Fn(),
) -> Measurement {
    run(home, invocation);
    sqlite_operation();
    let sqlite = median_duration(
        (0..SAMPLES)
            .map(|_| {
                let started = Instant::now();
                sqlite_operation();
                started.elapsed()
            })
            .collect(),
    );
    let first_row = median_duration(
        (0..SAMPLES)
            .map(|_| time_to_first_row(home, invocation).0)
            .collect(),
    );
    let total = median_duration((0..SAMPLES).map(|_| elapsed(home, invocation)).collect());
    let peak_rss_bytes = peak_rss(home, invocation);
    black_box(connection);
    Measurement {
        database_notes,
        operation: operation.to_string(),
        requested,
        sqlite: Some(sqlite),
        first_row,
        total,
        peak_rss_bytes,
    }
}

fn measure_mutation(
    home: &Path,
    database_notes: usize,
    operation: &str,
    requested: usize,
    invocation: impl Fn(usize) -> Invocation,
) -> Measurement {
    let first_row = median_duration(
        (0..SAMPLES)
            .map(|sample| time_to_first_row(home, &invocation(sample)).0)
            .collect(),
    );
    let total = median_duration(
        (0..SAMPLES)
            .map(|sample| elapsed(home, &invocation(sample + SAMPLES)))
            .collect(),
    );
    let peak_rss_bytes = peak_rss(home, &invocation(SAMPLES * 2));
    Measurement {
        database_notes,
        operation: operation.to_string(),
        requested,
        sqlite: None,
        first_row,
        total,
        peak_rss_bytes,
    }
}

fn populate(connection: &mut Connection, start: usize, end: usize) -> i64 {
    let started = Instant::now();
    let transaction = connection.transaction().unwrap();
    let mut revision: i64 = transaction
        .query_row(
            "SELECT revision FROM global_revision WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    {
        let mut insert_note = transaction
            .prepare(
                "INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            )
            .unwrap();
        let mut insert_change = transaction
            .prepare(
                "INSERT INTO note_changes(revision, note_id, operation)
                 VALUES (?1, ?2, 'add')",
            )
            .unwrap();
        let mut insert_tag = transaction
            .prepare("INSERT INTO note_tags(note_pk, tag) VALUES (?1, ?2)")
            .unwrap();
        for index in start..end {
            let id = fixture_id(index);
            let collection = if index % 10 == 0 {
                "bench/target"
            } else {
                "bench/corpus"
            };
            let timestamp = fixture_timestamp(index);
            revision += 1;
            insert_note
                .execute(params![
                    id,
                    collection,
                    fixture_body(index),
                    format!("Benchmark note {index}"),
                    timestamp,
                    revision,
                ])
                .unwrap();
            let pk = transaction.last_insert_rowid();
            insert_change
                .execute(params![revision, fixture_id(index)])
                .unwrap();
            if index % 4 == 0 {
                insert_tag.execute(params![pk, "benchmark"]).unwrap();
            }
        }
    }
    transaction
        .execute(
            "UPDATE global_revision SET revision = ?1 WHERE singleton = 1",
            [revision],
        )
        .unwrap();
    transaction.commit().unwrap();
    eprintln!(
        "populated {start}..{end} in {:.3}s",
        started.elapsed().as_secs_f64()
    );
    revision
}

fn sqlite_collection_read(connection: &Connection) {
    consume_note_query(
        connection,
        "WHERE n.collection = ?1 ORDER BY n.updated DESC, n.id DESC LIMIT ?2",
        params!["bench/target", 100_i64],
    );
}

fn sqlite_id_read(connection: &Connection, ids: &[String]) {
    let encoded = serde_json::to_string(ids).unwrap();
    consume_note_query(
        connection,
        "WHERE n.id IN (SELECT value FROM json_each(?1))
         ORDER BY n.updated DESC, n.id DESC",
        [encoded],
    );
}

fn consume_note_query(connection: &Connection, suffix: &str, parameters: impl rusqlite::Params) {
    let sql = format!(
        "SELECT n.pk, n.id, n.collection, n.body, n.title, n.created, n.updated,
                n.body_version, n.note_revision,
                COALESCE((SELECT json_group_array(tag ORDER BY tag)
                          FROM note_tags WHERE note_pk = n.pk), '[]'),
                COALESCE((SELECT json_group_array(target.id ORDER BY target.id)
                          FROM note_links
                          JOIN notes target ON target.pk = note_links.target_note_pk
                          WHERE note_links.note_pk = n.pk), '[]')
         FROM notes n {suffix}"
    );
    let mut statement = connection.prepare(&sql).unwrap();
    let mut rows = statement.query(parameters).unwrap();
    let mut count = 0;
    while let Some(row) = rows.next().unwrap() {
        for column in 0..11 {
            black_box(row.get_ref(column).unwrap());
        }
        count += 1;
    }
    black_box(count);
}

fn sqlite_recent_changes(connection: &Connection, revision: usize) {
    let mut statement = connection
        .prepare(
            "SELECT revision, note_id, operation FROM note_changes
             WHERE revision > ?1 ORDER BY revision ASC, note_id ASC",
        )
        .unwrap();
    let mut rows = statement.query([i64::try_from(revision).unwrap()]).unwrap();
    let mut count = 0;
    while let Some(row) = rows.next().unwrap() {
        black_box(row.get_ref(0).unwrap());
        black_box(row.get_ref(1).unwrap());
        black_box(row.get_ref(2).unwrap());
        count += 1;
    }
    black_box(count);
}

fn arbitrary_ids(note_count: usize, count: usize) -> Vec<String> {
    (0..count)
        .map(|offset| fixture_id(offset * 7919 % note_count))
        .collect()
}

fn fixture_id(index: usize) -> String {
    format!("019d0000-0000-7000-8000-{index:012x}")
}

fn fixture_timestamp(index: usize) -> String {
    let seconds = index % 60;
    let minutes = (index / 60) % 60;
    let hours = (index / 3_600) % 24;
    let days = (index / 86_400) % 28 + 1;
    format!("2026-07-{days:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn fixture_body(index: usize) -> String {
    format!(
        "# Benchmark note {index}\n\nCanonical CommonMark fixture body for deterministic lexical retrieval. {}",
        "agent storage ".repeat(8)
    )
}

fn note_count(connection: &Connection) -> usize {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    usize::try_from(count).unwrap()
}

fn run(home: &Path, invocation: &Invocation) {
    let status = spawn(home, invocation, Stdio::null(), Stdio::null())
        .wait()
        .unwrap();
    assert!(status.success(), "nt {:?} failed", invocation.arguments);
}

fn elapsed(home: &Path, invocation: &Invocation) -> Duration {
    let started = Instant::now();
    run(home, invocation);
    started.elapsed()
}

fn time_to_first_row(home: &Path, invocation: &Invocation) -> (Duration, Duration) {
    let started = Instant::now();
    let mut child = spawn(home, invocation, Stdio::piped(), Stdio::null());
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut first = Vec::new();
    let bytes = output.read_until(b'\n', &mut first).unwrap();
    let first_row = started.elapsed();
    assert!(bytes > 0, "nt {:?} produced no row", invocation.arguments);
    std::io::copy(&mut output, &mut std::io::sink()).unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "nt {:?} failed", invocation.arguments);
    (first_row, started.elapsed())
}

fn spawn(
    home: &Path,
    invocation: &Invocation,
    stdout: Stdio,
    stderr: Stdio,
) -> std::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home)
        .args(&invocation.arguments)
        .stdin(if invocation.stdin.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .unwrap();
    if !invocation.stdin.is_empty() {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&invocation.stdin)
            .unwrap();
    }
    child
}

fn peak_rss(home: &Path, invocation: &Invocation) -> u64 {
    let stderr_path = home.join("time.stderr");
    let stderr = fs::File::create(&stderr_path).unwrap();
    let mut command = Command::new("/usr/bin/time");
    #[cfg(target_os = "macos")]
    command.arg("-l");
    #[cfg(target_os = "linux")]
    command.args(["-f", "peak_rss_kib=%M"]);
    let mut child = command
        .arg(env!("CARGO_BIN_EXE_nt"))
        .args(&invocation.arguments)
        .env("HOME", home)
        .stdin(if invocation.stdin.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap();
    if !invocation.stdin.is_empty() {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&invocation.stdin)
            .unwrap();
    }
    assert!(child.wait().unwrap().success());
    let output = fs::read_to_string(stderr_path).unwrap();
    parse_peak_rss(&output)
}

#[cfg(target_os = "macos")]
fn parse_peak_rss(output: &str) -> u64 {
    output
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_suffix("maximum resident set size")
                .and_then(|value| value.trim().parse().ok())
        })
        .unwrap()
}

#[cfg(target_os = "linux")]
fn parse_peak_rss(output: &str) -> u64 {
    output
        .lines()
        .find_map(|line| line.strip_prefix("peak_rss_kib="))
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * 1024
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn parse_peak_rss(_output: &str) -> u64 {
    0
}

fn median_duration(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn database_bytes(database: &Path) -> u64 {
    [
        database.to_path_buf(),
        PathBuf::from(format!("{}-wal", database.display())),
        PathBuf::from(format!("{}-shm", database.display())),
    ]
    .iter()
    .filter_map(|path| fs::metadata(path).ok())
    .map(|metadata| metadata.len())
    .sum()
}
