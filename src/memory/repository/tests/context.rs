use super::*;

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
fn context_fallback_raw_evicts_an_overlapping_lexical_summary() {
    let mut repository = repository();
    let body = format!("recent {}", "x".repeat(1_017));
    for _ in 0..32 {
        repository.append(NewMemory::new(&body).unwrap()).unwrap();
    }
    repository
        .summarize(
            node(0, 0),
            NewSummary::new("needle overlapping summary").unwrap(),
        )
        .unwrap();

    let query = MemoryContextQuery::parse(&strings(&["needle"])).unwrap();
    let context = repository.context(&query).unwrap();

    assert!(
        context
            .iter()
            .all(|item| matches!(item, ContextItem::Raw(_)))
    );
    assert!(
        context
            .iter()
            .any(|item| matches!(item, ContextItem::Raw(memory) if memory.seq() == 16))
    );
    assert!(context_output_char_count(&context).unwrap() <= MEMORY_CONTEXT_CHARS);
}

#[test]
fn context_lexical_raw_candidates_are_bounded_and_prefer_newer_matches() {
    let mut repository = repository();
    repository
        .append(NewMemory::new("common common common common common").unwrap())
        .unwrap();
    for index in 1..300 {
        repository
            .append(NewMemory::new(format!("common memory {index}")).unwrap())
            .unwrap();
    }
    let query = MemoryContextQuery::parse(&strings(&["common"])).unwrap();

    let candidates =
        lexical_raw_candidates(&repository.connection, &query.fts_expression()).unwrap();
    assert_eq!(candidates.len(), 256);
    assert_eq!(candidates.first().unwrap().seq(), 300);
    assert_eq!(candidates.last().unwrap().seq(), 45);
    assert!(
        candidates
            .windows(2)
            .all(|pair| pair[0].seq() > pair[1].seq())
    );
    assert_eq!(
        candidates,
        lexical_raw_candidates(&repository.connection, &query.fts_expression()).unwrap()
    );
}

#[test]
fn context_lexical_summary_candidates_prefer_higher_levels_then_newer_blocks() {
    let repository = repository();
    for (level, block, summary) in [
        (0, 9, "common common common common common"),
        (1, 0, "common older block"),
        (2, 0, "common coarse summary"),
        (1, 1, "common newer block with extra document length"),
    ] {
        repository
            .connection
            .execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (?1, ?2, ?3, '2026-08-22T12:34:56Z')",
                rusqlite::params![level, block, summary],
            )
            .unwrap();
    }
    let query = MemoryContextQuery::parse(&strings(&["common"])).unwrap();

    let candidates =
        lexical_summary_candidates(&repository.connection, &query.fts_expression()).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|segment| (segment.node().level(), segment.node().block()))
            .collect::<Vec<_>>(),
        [(2, 0), (1, 1), (1, 0), (0, 9)]
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
fn queryless_context_uses_the_canonical_frontier_and_falls_back_to_children() {
    let mut repository = repository();
    append(&mut repository, 512, "history");
    for block in 0..16 {
        repository
            .connection
            .execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (0, ?1, ?2, '2026-08-22T12:34:56Z')",
                rusqlite::params![block, format!("child summary {block}")],
            )
            .unwrap();
    }
    repository
        .connection
        .execute(
            "INSERT INTO memory_segments(level, block, summary, created)
             VALUES (1, 0, 'canonical parent', '2026-08-22T12:34:56Z')",
            [],
        )
        .unwrap();

    let query = MemoryContextQuery::default();
    let context = repository.context(&query).unwrap();
    let summaries = context
        .iter()
        .filter_map(|item| match item {
            ContextItem::Summary(segment) => Some(segment.node()),
            ContextItem::Raw(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(summaries, [node(1, 0)]);
    assert!(context.iter().all(|item| match item {
        ContextItem::Raw(memory) => memory.seq() >= 257,
        ContextItem::Summary(segment) => {
            range(segment.node().level(), segment.node().block())
                .unwrap()
                .end()
                < 257
        }
    }));

    repository
        .connection
        .execute(
            "DELETE FROM memory_segments WHERE level = 1 AND block = 0",
            [],
        )
        .unwrap();
    let context = repository.context(&query).unwrap();
    let summaries = context
        .iter()
        .filter_map(|item| match item {
            ContextItem::Summary(segment) => Some(segment.node()),
            ContextItem::Raw(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        summaries,
        (0..16).map(|block| node(0, block)).collect::<Vec<_>>()
    );
    assert!(context_output_char_count(&context).unwrap() <= MEMORY_CONTEXT_CHARS);
}
