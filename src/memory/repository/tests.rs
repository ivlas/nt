use std::collections::BTreeSet;

use rusqlite::Connection;

use super::{ContextItem, ExpansionItem, Repository, context_output_char_count};
use crate::error::NtError;
use crate::memory::schema::OBJECTS;
use crate::memory::{
    MEMORY_CONTEXT_CHARS, MemoryContextQuery, MemoryListQuery, MemoryRecallQuery, NewMemory,
    NewSummary, SummaryNodeId,
};

fn repository() -> Repository {
    let connection = Connection::open_in_memory().unwrap();
    install_schema(&connection);
    Repository::from_connection(connection)
}

fn install_schema(connection: &Connection) {
    for object in OBJECTS {
        connection.execute_batch(object.sql).unwrap();
    }
}

fn append(repository: &mut Repository, count: usize, prefix: &str) {
    for index in 0..count {
        repository
            .append(NewMemory::new(format!("{prefix} {index}")).unwrap())
            .unwrap();
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn node(level: u64, block: u64) -> SummaryNodeId {
    SummaryNodeId::new(level, block).unwrap()
}

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
fn recall_uses_fts_with_bounds_limit_and_deterministic_sequence_ties() {
    let mut repository = repository();
    for body in ["alpha one", "unrelated", "alpha alpha three", "alpha four"] {
        repository.append(NewMemory::new(body).unwrap()).unwrap();
    }
    let query =
        MemoryRecallQuery::parse(&strings(&["alpha", "since:2", "until:4", "limit:2"])).unwrap();
    let recalled = repository.recall(&query).unwrap();
    assert_eq!(recalled.len(), 2);
    assert_eq!(recalled[0].seq(), 3);
    assert_eq!(recalled[1].seq(), 4);
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
fn context_is_bounded_never_truncates_deduplicates_and_is_deterministic() {
    let mut repository = repository();
    let body = format!("needle{}", "x".repeat(1_018));
    assert_eq!(body.chars().count(), 1_024);
    for _ in 0..40 {
        repository.append(NewMemory::new(&body).unwrap()).unwrap();
    }
    let query = MemoryContextQuery::parse(&strings(&["needle"])).unwrap();
    let first = repository.context(&query).unwrap();
    let second = repository.context(&query).unwrap();
    assert_eq!(first, second);
    assert!(context_output_char_count(&first).unwrap() <= MEMORY_CONTEXT_CHARS);

    let mut sequences = BTreeSet::new();
    let mut ordered = Vec::new();
    for item in &first {
        let ContextItem::Raw(memory) = item else {
            panic!("unexpected summary without summarized input");
        };
        assert_eq!(memory.body(), body);
        assert_eq!(item.content_char_count(), 1_024);
        assert!(sequences.insert(memory.seq()));
        ordered.push(memory.seq());
    }
    assert!(ordered.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn query_context_reallocates_unused_pools_to_recent_history() {
    let mut repository = repository();
    for index in 0..40 {
        let body = format!("recent {index} {}", "x".repeat(1_010));
        repository.append(NewMemory::new(body).unwrap()).unwrap();
    }

    let queryless = repository.context(&MemoryContextQuery::default()).unwrap();
    let obscure = repository
        .context(&MemoryContextQuery::parse(&strings(&["obscure-term"])).unwrap())
        .unwrap();
    assert_eq!(obscure, queryless);
    assert!(
        context_output_char_count(&obscure).unwrap() > MEMORY_CONTEXT_CHARS * 30 / 100,
        "unused lexical pools should be available to recent history"
    );
    assert!(context_output_char_count(&obscure).unwrap() <= MEMORY_CONTEXT_CHARS);
}

#[test]
fn context_prefers_exact_raw_and_excludes_overlapping_summaries() {
    let mut repository = repository();
    for index in 0..16 {
        repository
            .append(NewMemory::new(format!("needle raw {index}")).unwrap())
            .unwrap();
    }
    repository
        .summarize(
            node(0, 0),
            NewSummary::new("needle overlapping summary").unwrap(),
        )
        .unwrap();
    let query = MemoryContextQuery::parse(&strings(&["needle"])).unwrap();
    let context = repository.context(&query).unwrap();
    assert_eq!(context.len(), 16);
    assert!(
        context
            .iter()
            .all(|item| matches!(item, ContextItem::Raw(_)))
    );
}

#[test]
fn queryless_context_is_deterministic_and_uses_only_bounded_candidates() {
    let mut repository = repository();
    append(&mut repository, 300, "recent");
    repository
        .summarize(node(0, 0), NewSummary::new("coarse early history").unwrap())
        .unwrap();
    let query = MemoryContextQuery::default();
    let context = repository.context(&query).unwrap();
    assert_eq!(context, repository.context(&query).unwrap());
    assert!(context.len() <= 257);
    assert!(matches!(&context[0], ContextItem::Summary(segment) if segment.node() == node(0, 0)));
    assert!(matches!(&context[1], ContextItem::Raw(memory) if memory.seq() == 45));
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
    let mut repository = repository();
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
    assert_eq!(repository.status().unwrap().highest_seq(), Some(15));
    let indexed: i64 = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'rolled'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed, 0);

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

#[test]
fn status_reports_counts_levels_jobs_and_fts_readiness() {
    let mut repository = repository();
    let empty = repository.status().unwrap();
    assert_eq!(empty.raw_count(), 0);
    assert_eq!(empty.highest_seq(), None);
    assert_eq!(empty.highest_completed_level(), None);
    assert!(empty.raw_fts_ready());
    assert!(empty.summary_fts_ready());

    append(&mut repository, 16, "status");
    repository
        .summarize(node(0, 0), NewSummary::new("status summary").unwrap())
        .unwrap();
    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 16);
    assert_eq!(status.highest_seq(), Some(16));
    assert_eq!(status.summary_count(), 1);
    assert_eq!(status.pending_count(), 0);
    assert_eq!(status.highest_completed_level(), Some(0));
    assert!(status.raw_fts_ready());
    assert!(status.summary_fts_ready());
}

#[test]
#[ignore = "manual one-million-memory SQLite scale fixture"]
fn audit_one_million_memory_operations_and_database_size() {
    use std::time::Instant;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("million.sqlite3");
    let connection = Connection::open(&path).unwrap();
    install_schema(&connection);
    connection
        .execute_batch("PRAGMA journal_mode = WAL")
        .unwrap();

    let started = Instant::now();
    connection
        .execute_batch(
            "WITH RECURSIVE generated(seq) AS (
                 VALUES(1)
                 UNION ALL
                 SELECT seq + 1 FROM generated WHERE seq < 1000000
             )
             INSERT INTO memories(seq, body, created)
             SELECT seq,
                    printf('scale memory %d sqlite recall', seq),
                    '2026-08-22T12:34:56Z'
             FROM generated;
             WITH RECURSIVE blocks(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM blocks WHERE block < 15
             )
             INSERT INTO memory_segments(level, block, summary, created)
             SELECT 0, block, printf('scale summary block %d', block),
                    '2026-08-22T12:34:56Z'
             FROM blocks;
             WITH RECURSIVE jobs(block) AS (
                 VALUES(16)
                 UNION ALL
                 SELECT block + 1 FROM jobs WHERE block < 62499
             )
             INSERT INTO memory_summary_jobs(level, block)
             SELECT 0, block FROM jobs;
             INSERT INTO memory_summary_jobs(level, block) VALUES (1, 0);",
        )
        .unwrap();
    let populate = started.elapsed();
    let mut repository = Repository::from_connection(connection);

    let started = Instant::now();
    assert_eq!(
        repository
            .append(NewMemory::new("measured append").unwrap())
            .unwrap(),
        1_000_001
    );
    let append_time = started.elapsed();

    let started = Instant::now();
    assert_eq!(repository.get_memory(900_000).unwrap().seq(), 900_000);
    let show_time = started.elapsed();

    let started = Instant::now();
    let listed = repository
        .list_memories(
            &MemoryListQuery::parse(&strings(&["since:900000", "until:900100", "limit:50"]))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(listed.len(), 50);
    let list_time = started.elapsed();

    let started = Instant::now();
    let recalled = repository
        .recall(&MemoryRecallQuery::parse(&strings(&["sqlite", "recall", "limit:20"])).unwrap())
        .unwrap();
    assert_eq!(recalled.len(), 20);
    let recall_time = started.elapsed();

    let started = Instant::now();
    let context = repository
        .context(&MemoryContextQuery::parse(&strings(&["sqlite"])).unwrap())
        .unwrap();
    assert!(!context.is_empty());
    let context_time = started.elapsed();

    let started = Instant::now();
    assert_eq!(repository.expand(node(0, 0)).unwrap().len(), 16);
    let expand_time = started.elapsed();

    let started = Instant::now();
    assert_eq!(repository.pending(Some(5)).unwrap().len(), 5);
    let pending_time = started.elapsed();

    let started = Instant::now();
    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 1_000_001);
    assert_eq!(status.pending_count(), 62_485);
    let status_time = started.elapsed();

    repository
        .connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let bytes = std::fs::metadata(&path).unwrap().len();
    println!(
        "populate={populate:?} append={append_time:?} show={show_time:?} \
         list={list_time:?} recall={recall_time:?} context={context_time:?} \
         expand={expand_time:?} pending={pending_time:?} status={status_time:?} \
         database_bytes={bytes}"
    );
}

#[test]
#[ignore = "manual complete one-million-memory pyramid benchmark"]
fn audit_complete_million_memory_pyramid_queries() {
    use std::time::Instant;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("complete-pyramid.sqlite3");
    let connection = Connection::open(&path).unwrap();
    install_schema(&connection);
    connection
        .execute_batch("PRAGMA journal_mode = WAL")
        .unwrap();

    let started = Instant::now();
    connection
        .execute_batch(
            "WITH RECURSIVE generated(seq) AS (
                 VALUES(1)
                 UNION ALL
                 SELECT seq + 1 FROM generated WHERE seq < 1000000
             )
             INSERT INTO memories(seq, body, created)
             SELECT seq,
                    printf(
                        'common memory %d %s',
                        seq,
                        CASE WHEN seq = 777777 THEN 'selective' ELSE 'ordinary' END
                    ),
                    '2026-08-22T12:34:56Z'
             FROM generated;

             WITH RECURSIVE blocks(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM blocks WHERE block < 62499
             )
             INSERT INTO memory_segments(level, block, summary, created)
             SELECT 0,
                    block,
                    printf(
                        'common L0 summary %d %s',
                        block,
                        CASE WHEN block = 48611 THEN 'selective' ELSE 'ordinary' END
                    ),
                    '2026-08-22T12:34:56Z'
             FROM blocks;

             WITH RECURSIVE blocks(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM blocks WHERE block < 3905
             )
             INSERT INTO memory_segments(level, block, summary, created)
             SELECT 1,
                    block,
                    printf(
                        'common L1 summary %d %s',
                        block,
                        CASE WHEN block = 3038 THEN 'selective' ELSE 'ordinary' END
                    ),
                    '2026-08-22T12:34:56Z'
             FROM blocks;

             WITH RECURSIVE blocks(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM blocks WHERE block < 243
             )
             INSERT INTO memory_segments(level, block, summary, created)
             SELECT 2,
                    block,
                    printf(
                        'common L2 summary %d %s',
                        block,
                        CASE WHEN block = 189 THEN 'selective' ELSE 'ordinary' END
                    ),
                    '2026-08-22T12:34:56Z'
             FROM blocks;

             WITH RECURSIVE blocks(block) AS (
                 VALUES(0)
                 UNION ALL
                 SELECT block + 1 FROM blocks WHERE block < 14
             )
             INSERT INTO memory_segments(level, block, summary, created)
             SELECT 3,
                    block,
                    printf(
                        'common L3 summary %d %s',
                        block,
                        CASE WHEN block = 11 THEN 'selective' ELSE 'ordinary' END
                    ),
                    '2026-08-22T12:34:56Z'
             FROM blocks;",
        )
        .unwrap();
    let populate = started.elapsed();
    let repository = Repository::from_connection(connection);

    let started = Instant::now();
    let selective_recall = repository
        .recall(&MemoryRecallQuery::parse(&strings(&["selective", "limit:20"])).unwrap())
        .unwrap();
    assert_eq!(selective_recall.len(), 1);
    assert_eq!(selective_recall[0].seq(), 777_777);
    let selective_recall_time = started.elapsed();

    let started = Instant::now();
    let common_recall = repository
        .recall(&MemoryRecallQuery::parse(&strings(&["common", "limit:20"])).unwrap())
        .unwrap();
    assert_eq!(common_recall.len(), 20);
    let common_recall_time = started.elapsed();

    let started = Instant::now();
    let selective_context = repository
        .context(&MemoryContextQuery::parse(&strings(&["selective"])).unwrap())
        .unwrap();
    assert!(context_output_char_count(&selective_context).unwrap() <= MEMORY_CONTEXT_CHARS);
    let selective_context_time = started.elapsed();

    let started = Instant::now();
    let common_context = repository
        .context(&MemoryContextQuery::parse(&strings(&["common"])).unwrap())
        .unwrap();
    assert!(context_output_char_count(&common_context).unwrap() <= MEMORY_CONTEXT_CHARS);
    let common_context_time = started.elapsed();

    let started = Instant::now();
    assert_eq!(repository.expand(node(3, 11)).unwrap().len(), 16);
    let expand_time = started.elapsed();

    let status = repository.status().unwrap();
    assert_eq!(status.raw_count(), 1_000_000);
    assert_eq!(status.summary_count(), 66_665);
    assert_eq!(status.pending_count(), 0);
    assert_eq!(status.highest_completed_level(), Some(3));

    repository
        .connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let bytes = std::fs::metadata(&path).unwrap().len();
    println!(
        "populate={populate:?} selective_recall={selective_recall_time:?} \
         common_recall={common_recall_time:?} selective_context={selective_context_time:?} \
         common_context={common_context_time:?} expand={expand_time:?} database_bytes={bytes}"
    );
}
