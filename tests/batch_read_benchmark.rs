use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rusqlite::{Connection, params};

fn run(home: &Path, arguments: &[String]) {
    let status = Command::new(env!("CARGO_BIN_EXE_nt"))
        .env("HOME", home)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "nt {arguments:?} failed");
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
#[ignore = "manual process-level batch-read benchmark"]
fn compare_one_hundred_show_processes_with_one_batch_read() {
    let home = tempfile::tempdir().unwrap();
    run(home.path(), &["init".to_string()]);
    let mut connection = Connection::open(home.path().join(".nt/nt.sqlite3")).unwrap();
    let transaction = connection.transaction().unwrap();
    let ids = {
        let mut insert = transaction
            .prepare(
                "INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
                 VALUES (?1, 'inbox', ?2, ?3, '2026-01-01T00:00:00Z',
                         '2026-01-01T00:00:00Z', 1)",
            )
            .unwrap();
        (0..100)
            .map(|index| {
                let id = format!("018fbe0a-6c00-7000-8000-{index:012x}");
                insert
                    .execute(params![
                        id,
                        format!("# Note {index}\nBody"),
                        format!("Note {index}")
                    ])
                    .unwrap();
                id
            })
            .collect::<Vec<_>>()
    };
    transaction.commit().unwrap();

    let batch_arguments = std::iter::once("read".to_string())
        .chain(ids.iter().map(|id| format!("id:{id}")))
        .collect::<Vec<_>>();
    run(home.path(), &["show".to_string(), ids[0].clone()]);
    run(home.path(), &batch_arguments);

    let mut show_samples = Vec::new();
    let mut read_samples = Vec::new();
    for _ in 0..3 {
        let started = Instant::now();
        for id in &ids {
            run(home.path(), &["show".to_string(), id.clone()]);
        }
        show_samples.push(started.elapsed());

        let started = Instant::now();
        run(home.path(), &batch_arguments);
        read_samples.push(started.elapsed());
    }

    let shows = median(show_samples);
    let batch = median(read_samples);
    println!(
        "100 x nt show: {shows:?}\n1 x nt read (100 IDs): {batch:?}\nspeedup: {:.1}x",
        shows.as_secs_f64() / batch.as_secs_f64()
    );
}
