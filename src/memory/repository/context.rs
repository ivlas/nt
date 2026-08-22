use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, params};

use super::Repository;
use super::stored::{decode_memory, decode_segment, invalid_node, node_range, node_values};
use crate::error::{NtError, Result};
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
            Self::Raw(memory) => Ok(raw_context_header(memory)),
            Self::Summary(segment) => {
                let raw_range = node_range(segment.node())?;
                Ok(summary_context_header(
                    segment,
                    (raw_range.start(), raw_range.end()),
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
        match self {
            Self::Raw(memory) => Ok(raw_output_char_count(memory)),
            Self::Summary(segment) => {
                let raw_range = node_range(segment.node())?;
                Ok(summary_output_char_count(
                    segment,
                    (raw_range.start(), raw_range.end()),
                ))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn content_char_count(&self) -> usize {
        self.content().chars().count()
    }

    fn raw_bounds(&self) -> Result<(u64, u64)> {
        match self {
            Self::Raw(memory) => {
                let seq = memory_sequence(memory)?;
                Ok((seq, seq))
            }
            Self::Summary(segment) => {
                let raw_range = node_range(segment.node())?;
                Ok((raw_range.start(), raw_range.end()))
            }
        }
    }
}

#[derive(Default)]
struct ContextSelection {
    items: Vec<ContextItem>,
    raw_sequences: BTreeSet<i64>,
    ranges: Vec<(u64, u64)>,
    used_chars: usize,
}

impl ContextSelection {
    fn select_raw(&mut self, candidates: &[Memory], budget: usize) -> Result<()> {
        let mut pool_used = 0;
        for memory in candidates {
            if self.raw_sequences.contains(&memory.seq()) {
                continue;
            }
            let seq = memory_sequence(memory)?;
            let mut overlapping_summaries = BTreeSet::new();
            for (index, item) in self.items.iter().enumerate() {
                if matches!(item, ContextItem::Summary(_))
                    && ranges_overlap(item.raw_bounds()?, (seq, seq))
                {
                    overlapping_summaries.insert(index);
                }
            }
            let retained_count = self.items.len() - overlapping_summaries.len();
            let retained_chars = self
                .items
                .iter()
                .enumerate()
                .filter(|(index, _)| !overlapping_summaries.contains(index))
                .try_fold(0_usize, |total, (_, item)| {
                    Ok::<_, NtError>(total + item.output_char_count()?)
                })?
                + retained_count.saturating_sub(1);
            let chars = raw_output_char_count(memory) + usize::from(retained_count != 0);
            if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - retained_chars {
                continue;
            }

            if !overlapping_summaries.is_empty() {
                let mut index = 0;
                self.items.retain(|_| {
                    let retain = !overlapping_summaries.contains(&index);
                    index += 1;
                    retain
                });
                self.ranges = self
                    .items
                    .iter()
                    .map(ContextItem::raw_bounds)
                    .collect::<Result<Vec<_>>>()?;
                self.used_chars = retained_chars;
            }
            pool_used += chars;
            self.used_chars += chars;
            self.raw_sequences.insert(memory.seq());
            self.ranges.push((seq, seq));
            self.items.push(ContextItem::Raw(memory.clone()));
        }
        Ok(())
    }

    fn select_summaries(&mut self, candidates: &[MemorySegment], budget: usize) -> Result<()> {
        let mut pool_used = 0;
        for segment in candidates {
            let raw_range = node_range(segment.node())?;
            let bounds = (raw_range.start(), raw_range.end());
            if self
                .ranges
                .iter()
                .any(|selected| ranges_overlap(*selected, bounds))
            {
                continue;
            }
            let chars =
                summary_output_char_count(segment, bounds) + usize::from(!self.items.is_empty());
            if chars > budget - pool_used || chars > MEMORY_CONTEXT_CHARS - self.used_chars {
                continue;
            }
            pool_used += chars;
            self.used_chars += chars;
            self.ranges.push(bounds);
            self.items.push(ContextItem::Summary(segment.clone()));
        }
        Ok(())
    }

    fn fill_remaining(&mut self, recent: &[Memory], broad: &[MemorySegment]) -> Result<()> {
        let remaining = MEMORY_CONTEXT_CHARS - self.used_chars;
        let recent_budget = remaining * 60 / 100;
        self.select_raw(recent, recent_budget)?;
        self.select_summaries(broad, remaining - recent_budget)?;

        // If either fallback pool was sparse, let the other consume the residue.
        self.select_raw(recent, MEMORY_CONTEXT_CHARS)?;
        self.select_summaries(broad, MEMORY_CONTEXT_CHARS)
    }

    fn finish(self) -> Result<Vec<ContextItem>> {
        let mut ordered = self
            .items
            .into_iter()
            .map(|item| {
                let (start, end) = item.raw_bounds()?;
                let kind = matches!(item, ContextItem::Summary(_)) as u8;
                Ok(((start, end, kind), item))
            })
            .collect::<Result<Vec<_>>>()?;
        ordered.sort_by_key(|(key, _)| *key);
        let selected = ordered
            .into_iter()
            .map(|(_, item)| item)
            .collect::<Vec<_>>();
        if context_output_char_count(&selected)? > MEMORY_CONTEXT_CHARS {
            return Err(NtError::MemoryContextOverflow);
        }
        Ok(selected)
    }
}

fn raw_context_header(memory: &Memory) -> String {
    format!("# memory {} ({})\n", memory.seq(), memory.created())
}

fn summary_context_header(segment: &MemorySegment, bounds: (u64, u64)) -> String {
    format!("# summary {} ({}-{})\n", segment.node(), bounds.0, bounds.1)
}

fn raw_output_char_count(memory: &Memory) -> usize {
    raw_context_header(memory).chars().count() + memory.body().chars().count() + 1
}

fn summary_output_char_count(segment: &MemorySegment, bounds: (u64, u64)) -> usize {
    summary_context_header(segment, bounds).chars().count() + segment.summary().chars().count() + 1
}

fn memory_sequence(memory: &Memory) -> Result<u64> {
    u64::try_from(memory.seq()).map_err(|error| {
        NtError::invalid_stored_memory_with_source(format!("seq: {}", memory.seq()), "seq", error)
    })
}

impl Repository {
    pub(crate) fn context(&self, query: &MemoryContextQuery) -> Result<Vec<ContextItem>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut selection = ContextSelection::default();

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
            selection.select_raw(&recent, raw_budget)?;
            selection.select_summaries(&broad, MEMORY_CONTEXT_CHARS - raw_budget)?;
        } else {
            let lexical_raw_budget = MEMORY_CONTEXT_CHARS * 40 / 100;
            let recent_raw_budget = MEMORY_CONTEXT_CHARS * 30 / 100;
            selection.select_raw(
                &lexical_raw_candidates(&transaction, &query.fts_expression())?,
                lexical_raw_budget,
            )?;
            selection.select_raw(&recent, recent_raw_budget)?;
            selection.select_summaries(
                &lexical_summary_candidates(&transaction, &query.fts_expression())?,
                MEMORY_CONTEXT_CHARS - lexical_raw_budget - recent_raw_budget,
            )?;
        }

        selection.fill_remaining(&recent, &broad)?;
        let selected = selection.finish()?;
        transaction.commit()?;
        Ok(selected)
    }
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
    let highest_seq = memory_sequence(newest)?;
    let recent_start = memory_sequence(oldest)?;
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
        |level, first_block| -> Result<Option<u64>> {
            let node = SummaryNodeId::new(level, first_block)?;
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
            Ok(next_by_level.get(&node.level()).copied().flatten())
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

#[cfg(test)]
mod tests {
    use super::{ContextItem, ContextSelection, context_output_char_count, ranges_overlap};
    use crate::memory::{
        MEMORY_CONTEXT_CHARS, Memory, MemorySegment, NewMemory, NewSummary, SummaryNodeId,
    };
    use crate::note::Timestamp;

    fn timestamp() -> Timestamp {
        "2026-08-22T12:34:56Z".parse().unwrap()
    }

    fn memory(seq: i64, body: impl AsRef<str>) -> Memory {
        Memory::from_new(seq, NewMemory::new(body).unwrap(), timestamp()).unwrap()
    }

    fn summary(pk: i64, level: u64, block: u64, body: impl AsRef<str>) -> MemorySegment {
        MemorySegment::from_new(
            pk,
            SummaryNodeId::new(level, block).unwrap(),
            NewSummary::new(body).unwrap(),
            timestamp(),
        )
        .unwrap()
    }

    #[test]
    fn raw_replaces_an_overlapping_summary() {
        let mut selection = ContextSelection::default();
        selection
            .select_summaries(&[summary(1, 0, 0, "broad history")], MEMORY_CONTEXT_CHARS)
            .unwrap();
        selection
            .select_raw(&[memory(7, "exact evidence")], MEMORY_CONTEXT_CHARS)
            .unwrap();

        let items = selection.finish().unwrap();
        assert!(matches!(&items[..], [ContextItem::Raw(memory)] if memory.seq() == 7));
    }

    #[test]
    fn final_ranges_do_not_overlap() {
        let mut selection = ContextSelection::default();
        selection
            .select_summaries(
                &[
                    summary(1, 0, 0, "first block"),
                    summary(2, 0, 1, "second block"),
                ],
                MEMORY_CONTEXT_CHARS,
            )
            .unwrap();
        selection
            .select_raw(
                &[memory(16, "end of first"), memory(40, "later exact")],
                MEMORY_CONTEXT_CHARS,
            )
            .unwrap();

        let items = selection.finish().unwrap();
        let ranges = items
            .iter()
            .map(ContextItem::raw_bounds)
            .collect::<crate::error::Result<Vec<_>>>()
            .unwrap();
        assert!(ranges.iter().enumerate().all(|(index, range)| {
            ranges[index + 1..]
                .iter()
                .all(|other| !ranges_overlap(*range, *other))
        }));
    }

    #[test]
    fn final_output_stays_within_the_context_limit() {
        let body = "x".repeat(1_024);
        let candidates = (1..=40).map(|seq| memory(seq, &body)).collect::<Vec<_>>();
        let mut selection = ContextSelection::default();
        selection
            .select_raw(&candidates, MEMORY_CONTEXT_CHARS)
            .unwrap();

        let items = selection.finish().unwrap();
        assert!(context_output_char_count(&items).unwrap() <= MEMORY_CONTEXT_CHARS);
    }

    #[test]
    fn selection_is_deterministic() {
        let candidates = (1..=40)
            .rev()
            .map(|seq| memory(seq, format!("memory {seq}")))
            .collect::<Vec<_>>();
        let select = || {
            let mut selection = ContextSelection::default();
            selection
                .select_raw(&candidates, MEMORY_CONTEXT_CHARS)
                .unwrap();
            selection.finish().unwrap()
        };

        assert_eq!(select(), select());
    }

    #[test]
    fn finish_orders_items_chronologically() {
        let mut selection = ContextSelection::default();
        selection
            .select_raw(
                &[memory(20, "newest"), memory(18, "older")],
                MEMORY_CONTEXT_CHARS,
            )
            .unwrap();
        selection
            .select_summaries(&[summary(1, 0, 0, "earliest")], MEMORY_CONTEXT_CHARS)
            .unwrap();

        let items = selection.finish().unwrap();
        assert!(matches!(&items[0], ContextItem::Summary(segment) if segment.node().block() == 0));
        assert!(matches!(&items[1], ContextItem::Raw(memory) if memory.seq() == 18));
        assert!(matches!(&items[2], ContextItem::Raw(memory) if memory.seq() == 20));
    }

    #[test]
    fn oversized_candidate_pool_skips_whole_items_without_truncation() {
        let body = format!("needle{}", "x".repeat(1_018));
        let candidates = (1..=40).map(|seq| memory(seq, &body)).collect::<Vec<_>>();
        let mut selection = ContextSelection::default();
        selection
            .select_raw(&candidates, MEMORY_CONTEXT_CHARS)
            .unwrap();

        let items = selection.finish().unwrap();
        assert!(items.len() < candidates.len());
        assert!(items.iter().all(|item| item.content() == body));
    }
}
