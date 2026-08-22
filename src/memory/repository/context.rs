use std::collections::BTreeSet;

use rusqlite::params;

use super::{Repository, decode_memory, decode_segment, invalid_node, node_range};
use crate::error::Result;
use crate::memory::{MEMORY_CONTEXT_CHARS, Memory, MemoryContextQuery, MemorySegment};

const CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ContextItem {
    Raw(Memory),
    Summary(MemorySegment),
}

impl ContextItem {
    pub(crate) fn content_char_count(&self) -> usize {
        match self {
            Self::Raw(memory) => memory.body().chars().count(),
            Self::Summary(segment) => segment.summary().chars().count(),
        }
    }

    fn raw_bounds(&self) -> Result<(u64, u64)> {
        match self {
            Self::Raw(memory) => {
                let seq = u64::try_from(memory.seq())
                    .expect("validated memory sequences are positive integers");
                Ok((seq, seq))
            }
            Self::Summary(segment) => {
                let raw_range = node_range(segment.node())?;
                Ok((raw_range.start(), raw_range.end()))
            }
        }
    }
}

impl Repository {
    pub(crate) fn context(&self, query: &MemoryContextQuery) -> Result<Vec<ContextItem>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut selected = Vec::new();
        let mut selected_raw = BTreeSet::new();
        let mut selected_ranges = Vec::new();
        let mut total_used = 0;

        // Integer remainder goes to the last pool: 60/40 without terms, 40/30/30 with terms.
        if query.terms().is_empty() {
            let raw_budget = MEMORY_CONTEXT_CHARS * 60 / 100;
            select_raw(
                recent_raw_candidates(&transaction)?,
                raw_budget,
                &mut total_used,
                &mut selected_raw,
                &mut selected_ranges,
                &mut selected,
            );
            select_summaries(
                broad_summary_candidates(&transaction)?,
                MEMORY_CONTEXT_CHARS - raw_budget,
                &mut total_used,
                &mut selected_ranges,
                &mut selected,
            )?;
        } else {
            let lexical_raw_budget = MEMORY_CONTEXT_CHARS * 40 / 100;
            let recent_raw_budget = MEMORY_CONTEXT_CHARS * 30 / 100;
            select_raw(
                lexical_raw_candidates(&transaction, &query.fts_expression())?,
                lexical_raw_budget,
                &mut total_used,
                &mut selected_raw,
                &mut selected_ranges,
                &mut selected,
            );
            select_raw(
                recent_raw_candidates(&transaction)?,
                recent_raw_budget,
                &mut total_used,
                &mut selected_raw,
                &mut selected_ranges,
                &mut selected,
            );
            select_summaries(
                lexical_summary_candidates(&transaction, &query.fts_expression())?,
                MEMORY_CONTEXT_CHARS - lexical_raw_budget - recent_raw_budget,
                &mut total_used,
                &mut selected_ranges,
                &mut selected,
            )?;
        }

        selected.sort_by_key(|item| {
            let (start, end) = item
                .raw_bounds()
                .expect("selected context items have validated raw ranges");
            let kind = matches!(item, ContextItem::Summary(_)) as u8;
            (start, end, kind)
        });
        assert!(
            selected
                .iter()
                .map(ContextItem::content_char_count)
                .sum::<usize>()
                <= MEMORY_CONTEXT_CHARS
        );
        transaction.commit()?;
        Ok(selected)
    }
}

fn select_raw(
    candidates: Vec<Memory>,
    budget: usize,
    total_used: &mut usize,
    selected_raw: &mut BTreeSet<i64>,
    selected_ranges: &mut Vec<(u64, u64)>,
    selected: &mut Vec<ContextItem>,
) {
    let mut pool_used = 0;
    for memory in candidates {
        if selected_raw.contains(&memory.seq()) {
            continue;
        }
        let chars = memory.body().chars().count();
        if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - *total_used {
            continue;
        }
        let seq = u64::try_from(memory.seq()).expect("validated memory sequences are positive");
        pool_used += chars;
        *total_used += chars;
        selected_raw.insert(memory.seq());
        selected_ranges.push((seq, seq));
        selected.push(ContextItem::Raw(memory));
    }
}

fn select_summaries(
    candidates: Vec<MemorySegment>,
    budget: usize,
    total_used: &mut usize,
    selected_ranges: &mut Vec<(u64, u64)>,
    selected: &mut Vec<ContextItem>,
) -> Result<()> {
    let mut pool_used = 0;
    for segment in candidates {
        let raw_range = node_range(segment.node())?;
        let bounds = (raw_range.start(), raw_range.end());
        if selected_ranges
            .iter()
            .any(|selected| ranges_overlap(*selected, bounds))
        {
            continue;
        }
        let chars = segment.summary().chars().count();
        if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - *total_used {
            continue;
        }
        pool_used += chars;
        *total_used += chars;
        selected_ranges.push(bounds);
        selected.push(ContextItem::Summary(segment));
    }
    Ok(())
}

fn ranges_overlap(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 <= right.1 && right.0 <= left.1
}

fn recent_raw_candidates(connection: &rusqlite::Connection) -> Result<Vec<Memory>> {
    let mut statement = connection.prepare(&format!(
        "SELECT seq, body, created FROM memories ORDER BY seq DESC LIMIT {CANDIDATE_LIMIT}"
    ))?;
    let mut rows = statement.query([])?;
    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        candidates.push(decode_memory(row)?);
    }
    Ok(candidates)
}

fn lexical_raw_candidates(
    connection: &rusqlite::Connection,
    expression: &str,
) -> Result<Vec<Memory>> {
    let mut statement = connection.prepare(&format!(
        "SELECT m.seq, m.body, m.created
         FROM memory_fts
         JOIN memories m ON m.seq = memory_fts.rowid
         WHERE memory_fts MATCH ?1
         ORDER BY bm25(memory_fts) ASC, m.seq DESC
         LIMIT {CANDIDATE_LIMIT}"
    ))?;
    let mut rows = statement.query([expression])?;
    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        candidates.push(decode_memory(row)?);
    }
    Ok(candidates)
}

fn broad_summary_candidates(connection: &rusqlite::Connection) -> Result<Vec<MemorySegment>> {
    let mut statement = connection.prepare(&format!(
        "SELECT pk, level, block, summary, created FROM memory_segments
         ORDER BY level DESC, block DESC LIMIT {CANDIDATE_LIMIT}"
    ))?;
    let mut rows = statement.query([])?;
    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        let segment = decode_segment(row)?;
        node_range(segment.node())
            .map_err(|_| invalid_node(segment.node(), "has an invalid raw range"))?;
        candidates.push(segment);
    }
    Ok(candidates)
}

fn lexical_summary_candidates(
    connection: &rusqlite::Connection,
    expression: &str,
) -> Result<Vec<MemorySegment>> {
    let mut statement = connection.prepare(&format!(
        "SELECT s.pk, s.level, s.block, s.summary, s.created
         FROM memory_segment_fts
         JOIN memory_segments s ON s.pk = memory_segment_fts.rowid
         WHERE memory_segment_fts MATCH ?1
         ORDER BY bm25(memory_segment_fts) ASC, s.level DESC, s.block DESC
         LIMIT {CANDIDATE_LIMIT}"
    ))?;
    let mut rows = statement.query(params![expression])?;
    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        let segment = decode_segment(row)?;
        node_range(segment.node())
            .map_err(|_| invalid_node(segment.node(), "has an invalid raw range"))?;
        candidates.push(segment);
    }
    Ok(candidates)
}
