use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, params};

use super::{Repository, decode_memory, decode_segment, invalid_node, node_range, node_values};
use crate::error::Result;
use crate::memory::{
    MEMORY_CONTEXT_CHARS, Memory, MemoryContextQuery, MemorySegment, SummaryNodeId, frontier,
};

const CANDIDATE_LIMIT: usize = 256;
const FRONTIER_FETCH_BATCH_SIZE: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ContextItem {
    Raw(Memory),
    Summary(MemorySegment),
}

impl ContextItem {
    pub(crate) fn context_header(&self) -> Result<String> {
        match self {
            Self::Raw(memory) => Ok(format!(
                "# memory {} ({})\n",
                memory.seq(),
                memory.created()
            )),
            Self::Summary(segment) => {
                let raw_range = node_range(segment.node())?;
                Ok(format!(
                    "# summary {} ({}-{})\n",
                    segment.node(),
                    raw_range.start(),
                    raw_range.end()
                ))
            }
        }
    }

    pub(crate) fn content(&self) -> &str {
        match self {
            Self::Raw(memory) => memory.body(),
            Self::Summary(segment) => segment.summary(),
        }
    }

    pub(crate) fn output_char_count(&self) -> Result<usize> {
        Ok(self.context_header()?.chars().count() + self.content().chars().count() + 1)
    }

    #[cfg(test)]
    pub(crate) fn content_char_count(&self) -> usize {
        self.content().chars().count()
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

        let recent = recent_raw_candidates(&transaction)?;
        let broad = if query.terms().is_empty() {
            frontier_summary_candidates(&transaction, &recent)?
        } else {
            broad_summary_candidates(&transaction)?
        };

        // Preferred pools are 60/40 without terms and 40/30/30 with terms.
        // Remaining output capacity falls back to recent raw and broad summaries.
        if query.terms().is_empty() {
            let raw_budget = MEMORY_CONTEXT_CHARS * 60 / 100;
            select_raw(
                recent.clone(),
                raw_budget,
                &mut total_used,
                &mut selected_raw,
                &mut selected_ranges,
                &mut selected,
            )?;
            select_summaries(
                broad.clone(),
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
            )?;
            select_raw(
                recent.clone(),
                recent_raw_budget,
                &mut total_used,
                &mut selected_raw,
                &mut selected_ranges,
                &mut selected,
            )?;
            select_summaries(
                lexical_summary_candidates(&transaction, &query.fts_expression())?,
                MEMORY_CONTEXT_CHARS - lexical_raw_budget - recent_raw_budget,
                &mut total_used,
                &mut selected_ranges,
                &mut selected,
            )?;
        }

        fill_remaining(
            &recent,
            &broad,
            &mut total_used,
            &mut selected_raw,
            &mut selected_ranges,
            &mut selected,
        )?;

        selected.sort_by_key(|item| {
            let (start, end) = item
                .raw_bounds()
                .expect("selected context items have validated raw ranges");
            let kind = matches!(item, ContextItem::Summary(_)) as u8;
            (start, end, kind)
        });
        if context_output_char_count(&selected)? > MEMORY_CONTEXT_CHARS {
            return Err(crate::error::NtError::MemoryContextOverflow);
        }
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
) -> Result<()> {
    let mut pool_used = 0;
    for memory in candidates {
        if selected_raw.contains(&memory.seq()) {
            continue;
        }
        let item = ContextItem::Raw(memory);
        let chars = item.output_char_count()? + usize::from(!selected.is_empty());
        if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - *total_used {
            continue;
        }
        let ContextItem::Raw(memory) = &item else {
            unreachable!();
        };
        let seq = u64::try_from(memory.seq()).expect("validated memory sequences are positive");
        pool_used += chars;
        *total_used += chars;
        selected_raw.insert(memory.seq());
        selected_ranges.push((seq, seq));
        selected.push(item);
    }
    Ok(())
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
        let item = ContextItem::Summary(segment);
        let chars = item.output_char_count()? + usize::from(!selected.is_empty());
        if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - *total_used {
            continue;
        }
        pool_used += chars;
        *total_used += chars;
        selected_ranges.push(bounds);
        selected.push(item);
    }
    Ok(())
}

fn fill_remaining(
    recent: &[Memory],
    broad: &[MemorySegment],
    total_used: &mut usize,
    selected_raw: &mut BTreeSet<i64>,
    selected_ranges: &mut Vec<(u64, u64)>,
    selected: &mut Vec<ContextItem>,
) -> Result<()> {
    let remaining = MEMORY_CONTEXT_CHARS - *total_used;
    let recent_budget = remaining * 60 / 100;
    select_raw(
        recent.to_vec(),
        recent_budget,
        total_used,
        selected_raw,
        selected_ranges,
        selected,
    )?;
    select_summaries(
        broad.to_vec(),
        remaining - recent_budget,
        total_used,
        selected_ranges,
        selected,
    )?;

    // If either fallback pool was sparse, let the other consume the residue.
    select_raw(
        recent.to_vec(),
        MEMORY_CONTEXT_CHARS,
        total_used,
        selected_raw,
        selected_ranges,
        selected,
    )?;
    select_summaries(
        broad.to_vec(),
        MEMORY_CONTEXT_CHARS,
        total_used,
        selected_ranges,
        selected,
    )
}

pub(crate) fn context_output_char_count(items: &[ContextItem]) -> Result<usize> {
    let item_chars = items.iter().try_fold(0_usize, |total, item| {
        Ok::<_, crate::error::NtError>(total + item.output_char_count()?)
    })?;
    Ok(item_chars + items.len().saturating_sub(1))
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

pub(super) fn lexical_raw_candidates(
    connection: &rusqlite::Connection,
    expression: &str,
) -> Result<Vec<Memory>> {
    let mut statement = connection.prepare(&format!(
        "SELECT m.seq, m.body, m.created
         FROM memory_fts
         JOIN memories m ON m.seq = memory_fts.rowid
         WHERE memory_fts MATCH ?1
         ORDER BY m.seq DESC
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

fn frontier_summary_candidates(
    connection: &rusqlite::Connection,
    recent: &[Memory],
) -> Result<Vec<MemorySegment>> {
    let (Some(newest), Some(oldest)) = (recent.first(), recent.last()) else {
        return Ok(Vec::new());
    };
    let any_summaries =
        connection.query_row("SELECT EXISTS(SELECT 1 FROM memory_segments)", [], |row| {
            row.get::<_, bool>(0)
        })?;
    if !any_summaries {
        return Ok(Vec::new());
    }
    let highest_seq = u64::try_from(newest.seq()).expect("validated memory sequences are positive");
    let recent_start =
        u64::try_from(oldest.seq()).expect("validated memory sequences are positive");
    let mut next_by_level = BTreeMap::<u64, Option<u64>>::new();
    let mut next_summary = connection.prepare(
        "SELECT block FROM memory_segments
         WHERE level = ?1 AND block >= ?2
         ORDER BY block ASC LIMIT 1",
    )?;
    let nodes = frontier(
        highest_seq,
        recent_start,
        CANDIDATE_LIMIT,
        |node| -> Result<bool> {
            let (level, block) = node_values(node)?;
            let seek = next_by_level
                .get(&node.level())
                .is_none_or(|next| next.is_some_and(|next| next < node.block()));
            if seek {
                let next = next_summary
                    .query_row(params![level, block], |row| row.get::<_, i64>(0))
                    .optional()?
                    .map(|next| {
                        u64::try_from(next)
                            .map_err(|_| invalid_node(node, "has an invalid stored block"))
                    })
                    .transpose()?;
                next_by_level.insert(node.level(), next);
            }
            Ok(next_by_level.get(&node.level()).copied().flatten() == Some(node.block()))
        },
    )?;
    drop(next_summary);

    load_frontier_segments(connection, &nodes)
}

fn load_frontier_segments(
    connection: &rusqlite::Connection,
    nodes: &[SummaryNodeId],
) -> Result<Vec<MemorySegment>> {
    let mut by_node = BTreeMap::new();
    for batch in nodes.chunks(FRONTIER_FETCH_BATCH_SIZE) {
        let mut sql =
            String::from("SELECT pk, level, block, summary, created FROM memory_segments WHERE ");
        let mut values = Vec::with_capacity(batch.len() * 2);
        for (index, node) in batch.iter().enumerate() {
            if index != 0 {
                sql.push_str(" OR ");
            }
            sql.push_str("(level = ? AND block = ?)");
            let (level, block) = node_values(*node)?;
            values.push(level);
            values.push(block);
        }

        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(rusqlite::params_from_iter(values))?;
        while let Some(row) = rows.next()? {
            let segment = decode_segment(row)?;
            node_range(segment.node())
                .map_err(|_| invalid_node(segment.node(), "has an invalid raw range"))?;
            by_node.insert(segment.node(), segment);
        }
    }

    nodes
        .iter()
        .map(|node| {
            by_node
                .remove(node)
                .ok_or_else(|| invalid_node(*node, "summary not found"))
        })
        .collect()
}

pub(super) fn lexical_summary_candidates(
    connection: &rusqlite::Connection,
    expression: &str,
) -> Result<Vec<MemorySegment>> {
    let mut statement = connection.prepare(&format!(
        "SELECT s.pk, s.level, s.block, s.summary, s.created
         FROM memory_segment_fts
         JOIN memory_segments s ON s.pk = memory_segment_fts.rowid
         WHERE memory_segment_fts MATCH ?1
         ORDER BY s.level DESC, s.block DESC
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
