use super::*;

#[test]
fn append_assigns_monotonic_sequences_and_enqueues_completed_raw_ranges() {
    let mut repository = repository();
    append(&mut repository, 32, "entry");

    assert_eq!(repository.get_memory(1).unwrap().body(), "entry 0");
    assert_eq!(repository.get_memory(32).unwrap().body(), "entry 31");
    let jobs = repository.pending(None).unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].node(), node(0, 0));
    assert_eq!(jobs[0].raw_range().start(), 1);
    assert_eq!(jobs[0].raw_range().end(), 16);
    assert_eq!(jobs[1].node(), node(0, 1));
    assert_eq!(repository.pending(Some(1)).unwrap().len(), 1);
    assert!(repository.pending(Some(0)).is_err());
}

#[test]
fn append_does_not_enqueue_an_incomplete_raw_range() {
    let mut repository = repository();
    for seq in 99..=111 {
        repository
            .connection
            .execute(
                "INSERT INTO memories(seq, body, created) VALUES (?1, ?2, ?3)",
                rusqlite::params![seq, format!("sparse {seq}"), "2026-08-22T12:34:56Z"],
            )
            .unwrap();
    }

    assert_eq!(
        repository
            .append(NewMemory::new("boundary memory").unwrap())
            .unwrap(),
        112
    );
    assert!(repository.pending(None).unwrap().is_empty());
}

#[test]
fn persisted_memories_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("memory.sqlite3");
    {
        let connection = Connection::open(&path).unwrap();
        install_schema(&connection);
        let mut repository = Repository::from_connection(connection);
        repository
            .append(NewMemory::new("persistent body").unwrap())
            .unwrap();
    }

    let repository = Repository::from_connection(Connection::open(&path).unwrap());
    assert_eq!(repository.get_memory(1).unwrap().body(), "persistent body");
}

#[test]
fn list_and_visit_apply_inclusive_bounds_ascending_order_and_sql_limit() {
    let mut repository = repository();
    append(&mut repository, 10, "listed");
    let query = MemoryListQuery::parse(&strings(&["since:3", "until:8", "limit:3"])).unwrap();
    let listed = repository.list_memories(&query).unwrap();
    assert_eq!(
        listed.iter().map(|memory| memory.seq()).collect::<Vec<_>>(),
        [3, 4, 5]
    );

    let expected = NtError::InvalidValue {
        field: "test visit",
        value: "stop".to_string(),
    };
    let result = repository.visit_memories(&MemoryListQuery::default(), |memory| {
        if memory.seq() == 2 {
            return Err(NtError::InvalidValue {
                field: "test visit",
                value: "stop".to_string(),
            });
        }
        Ok(())
    });
    assert_eq!(result.unwrap_err().to_string(), expected.to_string());
    assert!(matches!(
        repository.get_memory(99),
        Err(NtError::MemoryNotFound(99))
    ));
}

#[test]
fn recall_uses_fts_with_bounds_sql_limit_and_sequence_order_only() {
    let mut repository = repository();
    for body in [
        format!("alpha {}", "padding ".repeat(100)),
        "alpha alpha alpha alpha alpha".to_string(),
        "alpha final".to_string(),
        "unrelated".to_string(),
    ] {
        repository.append(NewMemory::new(body).unwrap()).unwrap();
    }
    let query =
        MemoryRecallQuery::parse(&strings(&["alpha", "since:1", "until:3", "limit:2"])).unwrap();
    let recalled = repository.recall(&query).unwrap();
    assert_eq!(
        recalled
            .iter()
            .map(|memory| memory.seq())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(recalled, repository.recall(&query).unwrap());
}

#[test]
fn append_reports_retryable_writer_contention() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("busy.sqlite3");
    let first_connection = Connection::open(&path).unwrap();
    install_schema(&first_connection);
    let second_connection = Connection::open(&path).unwrap();
    second_connection
        .busy_timeout(std::time::Duration::from_millis(1))
        .unwrap();
    let mut first = Repository::from_connection(first_connection);
    let mut second = Repository::from_connection(second_connection);
    let transaction = first
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();

    assert!(matches!(
        second.append(NewMemory::new("contended").unwrap()),
        Err(NtError::DatabaseBusy)
    ));
    transaction.rollback().unwrap();
}

#[test]
fn failed_level_zero_job_creation_rolls_back_raw_and_fts_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failed-append.sqlite3");
    let connection = Connection::open(&path).unwrap();
    install_schema(&connection);
    let mut repository = Repository::from_connection(connection);
    append(&mut repository, 15, "boundary");
    repository
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_memory_job BEFORE INSERT ON memory_summary_jobs BEGIN
                 SELECT RAISE(ABORT, 'injected job failure');
             END",
        )
        .unwrap();

    assert!(
        repository
            .append(NewMemory::new("rolled back").unwrap())
            .is_err()
    );

    drop(repository);
    let repository = Repository::from_connection(Connection::open(&path).unwrap());
    assert_eq!(repository.status().unwrap().highest_seq(), Some(15));
    assert_eq!(repository.status().unwrap().raw_count(), 15);
    assert_eq!(repository.status().unwrap().pending_count(), 0);
    let indexed: i64 = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'rolled'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed, 0);

    drop(repository);
    let mut repository = Repository::from_connection(Connection::open(&path).unwrap());
    repository
        .connection
        .execute("DROP TRIGGER fail_memory_job", [])
        .unwrap();
    assert_eq!(
        repository
            .append(NewMemory::new("committed boundary").unwrap())
            .unwrap(),
        16
    );
    assert_eq!(repository.pending(None).unwrap().len(), 1);
}

#[test]
fn invalid_stored_memory_is_an_operational_error_with_safe_identity() {
    let mut repository = repository();
    repository
        .append(NewMemory::new("valid body").unwrap())
        .unwrap();
    repository
        .connection
        .execute_batch(
            "DROP TRIGGER memories_immutable_update;
             PRAGMA ignore_check_constraints = ON;
             UPDATE memories SET body = 'not\rnormalized' WHERE seq = 1",
        )
        .unwrap();

    let error = repository.get_memory(1).unwrap_err();
    assert!(matches!(
        &error,
        NtError::InvalidStoredMemory {
            identity,
            field: "body",
            ..
        } if identity == "seq: 1"
    ));
    assert_eq!(error.exit_code(), 1);
    assert!(!error.to_string().contains("not\rnormalized"));
}
