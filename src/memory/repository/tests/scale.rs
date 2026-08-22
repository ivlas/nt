use super::*;

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

#[test]
#[ignore = "manual queryless context scale benchmark at 10k, 100k, 1M, and 10M memories"]
fn benchmark_queryless_context_across_history_sizes() {
    use std::time::{Duration, Instant};

    fn populate_raw_history(connection: &Connection, raw_count: u64) {
        connection
            .execute(
                "WITH RECURSIVE generated(seq) AS (
                     VALUES(1)
                     UNION ALL
                     SELECT seq + 1 FROM generated WHERE seq < ?1
                 )
                 INSERT INTO memories(seq, body, created)
                 SELECT seq, printf('benchmark memory %d', seq), '2026-08-22T12:34:56Z'
                 FROM generated",
                [i64::try_from(raw_count).unwrap()],
            )
            .unwrap();
    }

    fn populate_summary_level(connection: &Connection, raw_count: u64, level: u64) -> bool {
        let Some(width) = span(level) else {
            return false;
        };
        let blocks = raw_count / width;
        if blocks == 0 {
            return false;
        }
        connection
            .execute(
                "WITH RECURSIVE generated(block) AS (
                     VALUES(0)
                     UNION ALL
                     SELECT block + 1 FROM generated WHERE block + 1 < ?2
                 )
                 INSERT INTO memory_segments(level, block, summary, created)
                 SELECT ?1,
                        block,
                        printf('benchmark L%d summary %d', ?1, block),
                        '2026-08-22T12:34:56Z'
                 FROM generated",
                rusqlite::params![
                    i64::try_from(level).unwrap(),
                    i64::try_from(blocks).unwrap()
                ],
            )
            .unwrap();
        true
    }

    fn measure(repository: &Repository) -> (Vec<ContextItem>, Duration) {
        let query = MemoryContextQuery::default();
        let warm = repository.context(&query).unwrap();
        assert!(context_output_char_count(&warm).unwrap() <= MEMORY_CONTEXT_CHARS);
        let mut best = Duration::MAX;
        for _ in 0..10 {
            let started = Instant::now();
            let context = repository.context(&query).unwrap();
            best = best.min(started.elapsed());
            assert_eq!(context, warm);
        }
        (warm, best)
    }

    let directory = tempfile::tempdir().unwrap();
    for raw_count in [10_000_u64, 100_000, 1_000_000, 10_000_000] {
        let path = directory
            .path()
            .join(format!("queryless-{raw_count}.sqlite3"));
        let connection = Connection::open(path).unwrap();
        install_schema(&connection);
        connection
            .execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF")
            .unwrap();
        let populate_started = Instant::now();
        populate_raw_history(&connection, raw_count);
        let raw_populate_time = populate_started.elapsed();
        let repository = Repository::from_connection(connection);
        let (_, no_summary_time) = measure(&repository);

        let sparse_block = (raw_count - 256) / 16 - 1;
        repository
            .connection
            .execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (0, ?1, 'sparse benchmark summary', '2026-08-22T12:34:56Z')",
                [i64::try_from(sparse_block).unwrap()],
            )
            .unwrap();
        let (sparse, sparse_time) = measure(&repository);
        assert!(sparse.iter().any(
            |item| matches!(item, ContextItem::Summary(segment) if segment.node() == node(0, sparse_block))
        ));
        repository
            .connection
            .execute(
                "DELETE FROM memory_segments WHERE level = 0 AND block = ?1",
                [i64::try_from(sparse_block).unwrap()],
            )
            .unwrap();

        assert!(populate_summary_level(&repository.connection, raw_count, 0));
        let (level_zero, level_zero_time) = measure(&repository);
        assert!(
            level_zero
                .iter()
                .any(|item| matches!(item, ContextItem::Summary(_)))
        );

        for level in 1_u64.. {
            if !populate_summary_level(&repository.connection, raw_count, level) {
                break;
            }
        }
        let (complete, complete_time) = measure(&repository);
        let frontier_summaries = complete
            .iter()
            .filter(|item| matches!(item, ContextItem::Summary(_)))
            .count();
        println!(
            "raw_count={raw_count} frontier_summaries={frontier_summaries} \
             raw_populate={raw_populate_time:?} no_summaries={no_summary_time:?} \
             sparse={sparse_time:?} level_zero_only={level_zero_time:?} \
             complete={complete_time:?}"
        );
    }
}
