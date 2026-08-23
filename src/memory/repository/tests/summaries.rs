use super::*;

#[test]
fn visit_pending_streams_in_order_applies_sql_limit_and_stops_on_visitor_error() {
    let repository = repository();
    repository
        .connection
        .execute_batch(
            "WITH RECURSIVE jobs(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM jobs WHERE block < 9999
             )
             INSERT INTO memory_summary_jobs(level, block)
             SELECT 0, block FROM jobs;",
        )
        .unwrap();

    let mut limited = Vec::new();
    repository
        .visit_pending(Some(3), |job| {
            limited.push(job.node());
            Ok(())
        })
        .unwrap();
    assert_eq!(limited, [node(0, 0), node(0, 1), node(0, 2)]);

    let mut visited = 0;
    let error = repository
        .visit_pending(None, |_| {
            visited += 1;
            Err(NtError::InvalidValue {
                field: "test visit",
                value: "stop".to_string(),
            })
        })
        .unwrap_err();
    assert_eq!(visited, 1);
    assert!(matches!(
        error,
        NtError::InvalidValue {
            field: "test visit",
            ..
        }
    ));
}

#[test]
fn pending_inspection_requires_a_job_and_all_sixteen_children() {
    let mut repository = repository();
    append(&mut repository, 16, "child");
    let inspected = repository.inspect_pending(node(0, 0)).unwrap();
    assert_eq!(inspected.len(), 16);
    assert!(
        inspected
            .iter()
            .all(|item| matches!(item, ExpansionItem::Raw(_)))
    );

    repository
        .connection
        .execute("DROP TRIGGER memories_immutable_delete", [])
        .unwrap();
    repository
        .connection
        .execute("DELETE FROM memories WHERE seq = 8", [])
        .unwrap();
    assert!(matches!(
        repository.inspect_pending(node(0, 0)),
        Err(NtError::InvalidValue {
            field: "memory node",
            ..
        })
    ));
    assert!(repository.inspect_pending(node(0, 1)).is_err());
}

#[test]
fn summarize_is_idempotent_rejects_conflicts_and_keeps_fts_synchronized() {
    let mut repository = repository();
    append(&mut repository, 16, "source");
    let node = node(0, 0);
    let summary = NewSummary::new("stable lexical summary").unwrap();
    repository.summarize(node, summary.clone()).unwrap();
    assert!(repository.pending(None).unwrap().is_empty());

    repository.summarize(node, summary).unwrap();
    assert_eq!(repository.status().unwrap().summary_count(), 1);
    let indexed = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM memory_segment_fts
             WHERE memory_segment_fts MATCH 'lexical'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(indexed, 1);

    let error = repository
        .summarize(node, NewSummary::new("secret replacement").unwrap())
        .unwrap_err();
    assert!(matches!(
        &error,
        NtError::InvalidValue {
            field: "memory summary",
            value,
        } if value == "conflicts with existing summary"
    ));
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn direct_summary_lookup_returns_the_exact_validated_segment() {
    let mut repository = repository();
    append(&mut repository, 16, "source");
    let target = node(0, 0);
    repository
        .summarize(
            target,
            NewSummary::new("stored summary\nwithout wrapper").unwrap(),
        )
        .unwrap();

    let segment = repository.get_summary(target).unwrap();
    assert_eq!(segment.node(), target);
    assert_eq!(segment.summary(), "stored summary\nwithout wrapper");
    assert!(matches!(
        repository.get_summary(node(0, 99)),
        Err(NtError::InvalidValue {
            field: "memory node",
            value,
        }) if value == "L0:99 summary not found"
    ));
}

#[test]
fn direct_summary_lookup_rejects_invalid_stored_values() {
    let mut repository = repository();
    append(&mut repository, 16, "source");
    let node = node(0, 0);
    repository
        .summarize(node, NewSummary::new("valid summary").unwrap())
        .unwrap();
    repository
        .connection
        .execute_batch(
            "DROP TRIGGER memory_segments_immutable_update;
             UPDATE memory_segments SET summary = 'not' || char(13) || 'normalized'
             WHERE level = 0 AND block = 0;",
        )
        .unwrap();

    let error = repository.get_summary(node).unwrap_err();
    assert!(matches!(
        &error,
        NtError::InvalidStoredMemory {
            identity,
            field: "summary",
            ..
        } if identity == "segment row: 1"
    ));
    assert_eq!(error.exit_code(), 1);
    assert!(!error.to_string().contains("not\rnormalized"));
}

#[test]
fn failed_summarization_leaves_the_job_and_summary_state_unchanged() {
    let mut repository = repository();
    append(&mut repository, 16, "source");
    repository
        .connection
        .execute("DROP TRIGGER memories_immutable_delete", [])
        .unwrap();
    repository
        .connection
        .execute("DELETE FROM memories WHERE seq = 16", [])
        .unwrap();

    assert!(
        repository
            .summarize(node(0, 0), NewSummary::new("incomplete").unwrap())
            .is_err()
    );
    assert_eq!(repository.pending(None).unwrap().len(), 1);
    assert_eq!(repository.status().unwrap().summary_count(), 0);
}

#[test]
fn sixteen_out_of_order_level_zero_summaries_enqueue_level_one() {
    let mut repository = repository();
    append(&mut repository, 256, "history");
    for block in (0..16).rev() {
        repository
            .summarize(
                node(0, block),
                NewSummary::new(format!("block {block}")).unwrap(),
            )
            .unwrap();
    }
    let jobs = repository.pending(None).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].node(), node(1, 0));

    repository
        .connection
        .execute(
            "DELETE FROM memory_summary_jobs WHERE level = 1 AND block = 0",
            [],
        )
        .unwrap();
    repository
        .summarize(node(0, 0), NewSummary::new("block 0").unwrap())
        .unwrap();
    assert_eq!(repository.pending(None).unwrap()[0].node(), node(1, 0));

    let children = repository.inspect_pending(node(1, 0)).unwrap();
    assert_eq!(children.len(), 16);
    assert!(
        children
            .iter()
            .all(|item| matches!(item, ExpansionItem::Summary(_)))
    );
}

#[test]
fn expand_returns_exact_children_and_rejects_missing_nodes() {
    let mut repository = repository();
    append(&mut repository, 16, "raw");
    repository
        .summarize(node(0, 0), NewSummary::new("level zero").unwrap())
        .unwrap();
    let expanded = repository.expand(node(0, 0)).unwrap();
    assert_eq!(expanded.len(), 16);
    assert!(matches!(&expanded[0], ExpansionItem::Raw(memory) if memory.seq() == 1));
    assert!(expanded.iter().all(|item| {
        !matches!(item, ExpansionItem::Summary(segment) if segment.node() == node(0, 0))
    }));
    assert!(repository.expand(node(0, 1)).is_err());
}

#[test]
fn invalidation_removes_dependent_ancestors_but_preserves_raw_memory() {
    let mut repository = repository();
    append(&mut repository, 256, "immutable raw");
    for block in 0..16 {
        repository
            .summarize(
                node(0, block),
                NewSummary::new(format!("child {block}")).unwrap(),
            )
            .unwrap();
    }
    repository
        .summarize(node(1, 0), NewSummary::new("ancestor").unwrap())
        .unwrap();
    assert_eq!(repository.expand(node(1, 0)).unwrap().len(), 16);

    repository.invalidate(node(0, 3)).unwrap();
    assert!(repository.expand(node(0, 3)).is_err());
    assert!(repository.expand(node(1, 0)).is_err());
    assert_eq!(
        repository.get_memory(49).unwrap().body(),
        "immutable raw 48"
    );
    let jobs = repository.pending(None).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].node(), node(0, 3));
    let ancestor_fts = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM memory_segment_fts
             WHERE memory_segment_fts MATCH 'ancestor'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(ancestor_fts, 0);
    assert!(repository.invalidate(node(0, 3)).is_err());
}

#[test]
fn invalidation_removes_a_parent_job_that_is_no_longer_ready() {
    let mut repository = repository();
    append(&mut repository, 256, "raw");
    for block in 0..16 {
        repository
            .summarize(
                node(0, block),
                NewSummary::new(format!("child {block}")).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(repository.pending(None).unwrap()[0].node(), node(1, 0));

    repository.invalidate(node(0, 3)).unwrap();
    let jobs = repository.pending(None).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].node(), node(0, 3));
}

#[test]
fn repeated_expansion_recovers_exact_raw_evidence() {
    let mut repository = repository();
    append(&mut repository, 256, "evidence");
    for block in 0..16 {
        repository
            .summarize(
                node(0, block),
                NewSummary::new(format!("summary {block}")).unwrap(),
            )
            .unwrap();
    }
    repository
        .summarize(node(1, 0), NewSummary::new("coarse summary").unwrap())
        .unwrap();

    let level_zero = repository.expand(node(1, 0)).unwrap();
    assert_eq!(level_zero.len(), 16);
    assert!(level_zero.iter().all(|item| {
        !matches!(item, ExpansionItem::Summary(segment) if segment.node() == node(1, 0))
    }));
    assert!(
        matches!(&level_zero[0], ExpansionItem::Summary(segment) if segment.node() == node(0, 0))
    );
    let raw = repository.expand(node(0, 0)).unwrap();
    assert_eq!(raw.len(), 16);
    assert!(
        matches!(&raw[0], ExpansionItem::Raw(memory) if memory.seq() == 1 && memory.body() == "evidence 0")
    );
}

#[test]
fn failed_summary_transaction_has_no_persisted_segment_or_fts_state_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failed-summary.sqlite3");
    let connection = Connection::open(&path).unwrap();
    install_schema(&connection);
    let mut repository = Repository::from_connection(connection);
    append(&mut repository, 16, "summary source");
    repository
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_summary_job_delete
             BEFORE DELETE ON memory_summary_jobs
             WHEN OLD.level = 0 AND OLD.block = 0
             BEGIN
                 SELECT RAISE(ABORT, 'injected summary failure');
             END;",
        )
        .unwrap();

    assert!(
        repository
            .summarize(
                node(0, 0),
                NewSummary::new("rolled back derived summary").unwrap(),
            )
            .is_err()
    );
    drop(repository);

    let repository = Repository::from_connection(Connection::open(&path).unwrap());
    let status = repository.status().unwrap();
    assert_eq!(status.summary_count(), 0);
    assert_eq!(status.pending_count(), 1);
    assert_eq!(repository.pending(None).unwrap()[0].node(), node(0, 0));
    let indexed: i64 = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM memory_segment_fts
             WHERE memory_segment_fts MATCH 'rolled'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed, 0);
}

#[test]
fn failed_invalidation_restores_descendants_ancestors_and_fts_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failed-invalidation.sqlite3");
    let connection = Connection::open(&path).unwrap();
    install_schema(&connection);
    let mut repository = Repository::from_connection(connection);
    append(&mut repository, 256, "invalidation source");
    for block in 0..16 {
        repository
            .summarize(
                node(0, block),
                NewSummary::new(format!("durable child {block}")).unwrap(),
            )
            .unwrap();
    }
    repository
        .summarize(node(1, 0), NewSummary::new("durable ancestor").unwrap())
        .unwrap();
    repository
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_ancestor_delete
             BEFORE DELETE ON memory_segments
             WHEN OLD.level = 1 AND OLD.block = 0
             BEGIN
                 SELECT RAISE(ABORT, 'injected invalidation failure');
             END;",
        )
        .unwrap();

    assert!(repository.invalidate(node(0, 3)).is_err());
    drop(repository);

    let repository = Repository::from_connection(Connection::open(&path).unwrap());
    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 256);
    assert_eq!(status.summary_count(), 17);
    assert_eq!(status.pending_count(), 0);
    assert_eq!(repository.expand(node(0, 3)).unwrap().len(), 16);
    assert_eq!(repository.expand(node(1, 0)).unwrap().len(), 16);
    for term in ["child", "ancestor"] {
        let indexed: i64 = repository
            .connection
            .query_row(
                "SELECT COUNT(*) FROM memory_segment_fts
                 WHERE memory_segment_fts MATCH ?1",
                [term],
                |row| row.get(0),
            )
            .unwrap();
        assert!(indexed > 0);
    }
}
