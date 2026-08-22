use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use super::{
    Children, Memory, MemoryListQuery, MemoryRecallQuery, MemorySegment, NewMemory, NewSummary,
    RawRange, SummaryNodeId, children, level0_for_seq, parent, range,
};
use crate::error::{NtError, Result};
use crate::note::{Timestamp, timestamp_now};

mod context;
#[cfg(test)]
mod tests;

pub(crate) use context::ContextItem;
#[cfg(test)]
pub(crate) use context::context_output_char_count;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingJob {
    node: SummaryNodeId,
    raw_range: RawRange,
}

impl PendingJob {
    pub(crate) fn node(&self) -> SummaryNodeId {
        self.node
    }

    pub(crate) fn raw_range(&self) -> RawRange {
        self.raw_range
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExpansionItem {
    Raw(Memory),
    Summary(MemorySegment),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryStatus {
    raw_count: u64,
    highest_seq: Option<i64>,
    summary_count: u64,
    pending_count: u64,
    highest_completed_level: Option<u64>,
    raw_fts_ready: bool,
    summary_fts_ready: bool,
}

impl MemoryStatus {
    pub(crate) fn raw_count(&self) -> u64 {
        self.raw_count
    }

    pub(crate) fn highest_seq(&self) -> Option<i64> {
        self.highest_seq
    }

    pub(crate) fn summary_count(&self) -> u64 {
        self.summary_count
    }

    pub(crate) fn pending_count(&self) -> u64 {
        self.pending_count
    }

    pub(crate) fn highest_completed_level(&self) -> Option<u64> {
        self.highest_completed_level
    }

    pub(crate) fn raw_fts_ready(&self) -> bool {
        self.raw_fts_ready
    }

    pub(crate) fn summary_fts_ready(&self) -> bool {
        self.summary_fts_ready
    }
}

pub(crate) struct Repository {
    connection: Connection,
}

impl Repository {
    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }

    pub(crate) fn append(&mut self, memory: NewMemory) -> Result<i64> {
        let created = timestamp_now()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO memories(body, created) VALUES (?1, ?2)",
            params![memory.body(), created.as_str()],
        )?;
        let seq = transaction.last_insert_rowid();
        if seq % 16 == 0 {
            let node = level0_for_seq(seq).ok_or_else(|| invalid_node_value("invalid sequence"))?;
            let (level, block) = node_values(node)?;
            transaction.execute(
                "INSERT INTO memory_summary_jobs(level, block)
                 SELECT ?1, ?2
                 WHERE NOT EXISTS (
                     SELECT 1 FROM memory_segments WHERE level = ?1 AND block = ?2
                 )
                 ON CONFLICT(level, block) DO NOTHING",
                params![level, block],
            )?;
        }
        transaction.commit()?;
        Ok(seq)
    }

    pub(crate) fn get_memory(&self, seq: i64) -> Result<Memory> {
        let mut statement = self
            .connection
            .prepare("SELECT seq, body, created FROM memories WHERE seq = ?1")?;
        let mut rows = statement.query([seq])?;
        let Some(row) = rows.next()? else {
            return Err(missing_memory(seq));
        };
        decode_memory(row)
    }

    #[cfg(test)]
    pub(crate) fn list_memories(&self, query: &MemoryListQuery) -> Result<Vec<Memory>> {
        let mut memories = Vec::new();
        self.visit_memories(query, |memory| {
            memories.push(memory);
            Ok(())
        })?;
        Ok(memories)
    }

    pub(crate) fn visit_memories(
        &self,
        query: &MemoryListQuery,
        mut visit: impl FnMut(Memory) -> Result<()>,
    ) -> Result<()> {
        let mut sql = String::from("SELECT seq, body, created FROM memories");
        let mut clauses = Vec::new();
        let mut values = Vec::new();
        if let Some(since) = query.since() {
            values.push(since);
            clauses.push(format!("seq >= ?{}", values.len()));
        }
        if let Some(until) = query.until() {
            values.push(until);
            clauses.push(format!("seq <= ?{}", values.len()));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY seq ASC");
        if let Some(limit) = query.limit() {
            values.push(limit);
            sql.push_str(&format!(" LIMIT ?{}", values.len()));
        }

        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(rusqlite::params_from_iter(values))?;
        while let Some(row) = rows.next()? {
            visit(decode_memory(row)?)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn recall(&self, query: &MemoryRecallQuery) -> Result<Vec<Memory>> {
        let mut memories = Vec::new();
        self.visit_recalled(query, |memory| {
            memories.push(memory);
            Ok(())
        })?;
        Ok(memories)
    }

    pub(crate) fn visit_recalled(
        &self,
        query: &MemoryRecallQuery,
        mut visit: impl FnMut(Memory) -> Result<()>,
    ) -> Result<()> {
        let mut sql = String::from(
            "SELECT m.seq, m.body, m.created
             FROM memory_fts
             JOIN memories m ON m.seq = memory_fts.rowid
             WHERE memory_fts MATCH ?1",
        );
        let mut values = vec![rusqlite::types::Value::Text(query.fts_expression())];
        if let Some(since) = query.since() {
            values.push(since.into());
            sql.push_str(&format!(" AND m.seq >= ?{}", values.len()));
        }
        if let Some(until) = query.until() {
            values.push(until.into());
            sql.push_str(&format!(" AND m.seq <= ?{}", values.len()));
        }
        sql.push_str(" ORDER BY bm25(memory_fts) ASC, m.seq ASC");
        if let Some(limit) = query.limit() {
            values.push(limit.into());
            sql.push_str(&format!(" LIMIT ?{}", values.len()));
        }

        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(rusqlite::params_from_iter(values))?;
        while let Some(row) = rows.next()? {
            visit(decode_memory(row)?)?;
        }
        Ok(())
    }

    pub(crate) fn pending(&self, limit: Option<i64>) -> Result<Vec<PendingJob>> {
        if limit.is_some_and(|value| value <= 0) {
            return Err(NtError::InvalidValue {
                field: "memory pending limit",
                value: "must be positive".to_string(),
            });
        }
        let sql = if limit.is_some() {
            "SELECT level, block FROM memory_summary_jobs
             ORDER BY level ASC, block ASC LIMIT ?1"
        } else {
            "SELECT level, block FROM memory_summary_jobs
             ORDER BY level ASC, block ASC"
        };
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = if let Some(limit) = limit {
            statement.query([limit])?
        } else {
            statement.query([])?
        };
        let mut jobs = Vec::new();
        while let Some(row) = rows.next()? {
            let node = decode_node(row.get(0)?, row.get(1)?)?;
            jobs.push(PendingJob {
                node,
                raw_range: node_range(node)?,
            });
        }
        Ok(jobs)
    }

    pub(crate) fn inspect_pending(&self, node: SummaryNodeId) -> Result<Vec<ExpansionItem>> {
        let transaction = self.connection.unchecked_transaction()?;
        if !job_exists(&transaction, node)? {
            return Err(invalid_node(node, "is not pending"));
        }
        let children = load_exact_children(&transaction, node)?;
        transaction.commit()?;
        Ok(children)
    }

    pub(crate) fn summarize(&mut self, node: SummaryNodeId, summary: NewSummary) -> Result<()> {
        let created = timestamp_now()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (level, block) = node_values(node)?;
        let existing = transaction
            .query_row(
                "SELECT summary FROM memory_segments WHERE level = ?1 AND block = ?2",
                params![level, block],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing
            .as_deref()
            .is_some_and(|stored| stored != summary.summary())
        {
            return Err(NtError::InvalidValue {
                field: "memory summary",
                value: "conflicts with existing summary".to_string(),
            });
        }
        if existing.is_none() && !job_exists(&transaction, node)? {
            return Err(invalid_node(node, "is not pending"));
        }
        load_exact_children(&transaction, node)?;

        if existing.is_none() {
            transaction.execute(
                "INSERT INTO memory_segments(level, block, summary, created)
                 VALUES (?1, ?2, ?3, ?4)",
                params![level, block, summary.summary(), created.as_str()],
            )?;
        }
        transaction.execute(
            "DELETE FROM memory_summary_jobs WHERE level = ?1 AND block = ?2",
            params![level, block],
        )?;
        repair_parent_job(&transaction, node)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn expand(&self, node: SummaryNodeId) -> Result<Vec<ExpansionItem>> {
        let transaction = self.connection.unchecked_transaction()?;
        if !summary_exists(&transaction, node)? {
            return Err(invalid_node(node, "summary not found"));
        }
        let children = load_exact_children(&transaction, node)?;
        transaction.commit()?;
        Ok(children)
    }

    pub(crate) fn invalidate(&mut self, node: SummaryNodeId) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !summary_exists(&transaction, node)? {
            return Err(invalid_node(node, "summary not found"));
        }
        let max_level = transaction
            .query_row(
                "SELECT MAX(level) FROM (
                     SELECT level FROM memory_segments
                     UNION ALL
                     SELECT level FROM memory_summary_jobs
                 )",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )?
            .and_then(|level| u64::try_from(level).ok())
            .ok_or_else(|| invalid_node(node, "has invalid stored ancestry"))?;

        let mut current = node;
        loop {
            node_range(current)?;
            let (level, block) = node_values(current)?;
            transaction.execute(
                "DELETE FROM memory_summary_jobs WHERE level = ?1 AND block = ?2",
                params![level, block],
            )?;
            transaction.execute(
                "DELETE FROM memory_segments WHERE level = ?1 AND block = ?2",
                params![level, block],
            )?;
            if current.level() >= max_level {
                break;
            }
            current = parent(current.level(), current.block())
                .ok_or_else(|| invalid_node(current, "has invalid stored ancestry"))?;
        }

        if children_complete(&transaction, node)? {
            let (level, block) = node_values(node)?;
            transaction.execute(
                "INSERT INTO memory_summary_jobs(level, block) VALUES (?1, ?2)
                 ON CONFLICT(level, block) DO NOTHING",
                params![level, block],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn status(&self) -> Result<MemoryStatus> {
        let values = self.connection.query_row(
            "SELECT
                 COALESCE((SELECT MAX(seq) FROM memories), 0),
                 (SELECT MAX(seq) FROM memories),
                 (SELECT COUNT(*) FROM memory_segments),
                 (SELECT COUNT(*) FROM memory_summary_jobs),
                  (SELECT MAX(level) FROM memory_segments),
                  EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'memory_fts'),
                  EXISTS(SELECT 1 FROM sqlite_schema
                         WHERE type = 'table' AND name = 'memory_segment_fts')",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )?;
        let raw_count = nonnegative_count(values.0, "raw count")?;
        let summary_count = nonnegative_count(values.2, "summary count")?;
        Ok(MemoryStatus {
            raw_count,
            highest_seq: values.1,
            summary_count,
            pending_count: nonnegative_count(values.3, "pending count")?,
            highest_completed_level: values
                .4
                .map(|level| {
                    u64::try_from(level).map_err(|_| invalid_node_value("invalid stored level"))
                })
                .transpose()?,
            raw_fts_ready: values.5 != 0,
            summary_fts_ready: values.6 != 0,
        })
    }
}

fn load_exact_children(connection: &Connection, node: SummaryNodeId) -> Result<Vec<ExpansionItem>> {
    let expected = children(node.level(), node.block())
        .ok_or_else(|| invalid_node(node, "has an invalid child range"))?;
    let mut items = Vec::with_capacity(16);
    match expected {
        Children::Raw(raw_range) => {
            let start = raw_bound(raw_range.start(), node)?;
            let end = raw_bound(raw_range.end(), node)?;
            let mut statement = connection.prepare(
                "SELECT seq, body, created FROM memories
                 WHERE seq BETWEEN ?1 AND ?2 ORDER BY seq ASC LIMIT 17",
            )?;
            let mut rows = statement.query(params![start, end])?;
            let mut expected_seq = start;
            while let Some(row) = rows.next()? {
                let memory = decode_memory(row)?;
                if memory.seq() != expected_seq {
                    return Err(invalid_node(node, "has incomplete raw children"));
                }
                items.push(ExpansionItem::Raw(memory));
                expected_seq = expected_seq
                    .checked_add(1)
                    .ok_or_else(|| invalid_node(node, "has an invalid child range"))?;
            }
        }
        Children::Nodes(nodes) => {
            let child_level = i64::try_from(nodes[0].level())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let first_block = i64::try_from(nodes[0].block())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let last_block = i64::try_from(nodes[15].block())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let mut statement = connection.prepare(
                "SELECT pk, level, block, summary, created FROM memory_segments
                 WHERE level = ?1 AND block BETWEEN ?2 AND ?3
                 ORDER BY block ASC LIMIT 17",
            )?;
            let mut rows = statement.query(params![child_level, first_block, last_block])?;
            while let Some(row) = rows.next()? {
                let segment = decode_segment(row)?;
                let expected_node = nodes[items.len()];
                if segment.node() != expected_node {
                    return Err(invalid_node(node, "has incomplete summary children"));
                }
                items.push(ExpansionItem::Summary(segment));
            }
        }
    }
    if items.len() != 16 {
        return Err(invalid_node(node, "has incomplete children"));
    }
    Ok(items)
}

fn repair_parent_job(connection: &Connection, node: SummaryNodeId) -> Result<()> {
    let Some(parent_node) = parent(node.level(), node.block()) else {
        return Ok(());
    };
    if range(parent_node.level(), parent_node.block()).is_none() {
        return Ok(());
    }
    let (level, block) = node_values(parent_node)?;
    connection.execute(
        "DELETE FROM memory_summary_jobs WHERE level = ?1 AND block = ?2",
        params![level, block],
    )?;
    if children_complete(connection, parent_node)? && !summary_exists(connection, parent_node)? {
        connection.execute(
            "INSERT INTO memory_summary_jobs(level, block) VALUES (?1, ?2)
             ON CONFLICT(level, block) DO NOTHING",
            params![level, block],
        )?;
    }
    Ok(())
}

fn children_complete(connection: &Connection, node: SummaryNodeId) -> Result<bool> {
    match children(node.level(), node.block())
        .ok_or_else(|| invalid_node(node, "has an invalid child range"))?
    {
        Children::Raw(raw_range) => {
            let start = raw_bound(raw_range.start(), node)?;
            let end = raw_bound(raw_range.end(), node)?;
            let count = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE seq BETWEEN ?1 AND ?2",
                params![start, end],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(count == 16)
        }
        Children::Nodes(nodes) => {
            let level = i64::try_from(nodes[0].level())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let first = i64::try_from(nodes[0].block())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let last = i64::try_from(nodes[15].block())
                .map_err(|_| invalid_node(node, "has an invalid child range"))?;
            let count = connection.query_row(
                "SELECT COUNT(*) FROM memory_segments
                 WHERE level = ?1 AND block BETWEEN ?2 AND ?3",
                params![level, first, last],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(count == 16)
        }
    }
}

fn job_exists(connection: &Connection, node: SummaryNodeId) -> Result<bool> {
    let (level, block) = node_values(node)?;
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM memory_summary_jobs WHERE level = ?1 AND block = ?2
             )",
            params![level, block],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn summary_exists(connection: &Connection, node: SummaryNodeId) -> Result<bool> {
    let (level, block) = node_values(node)?;
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM memory_segments WHERE level = ?1 AND block = ?2
             )",
            params![level, block],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

pub(super) fn decode_memory(row: &Row<'_>) -> Result<Memory> {
    let seq = row.get::<_, i64>(0).map_err(|error| {
        NtError::invalid_stored_memory_with_source("identity: unknown", "seq", error)
    })?;
    let identity = format!("seq: {seq}");
    let body = row
        .get::<_, String>(1)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "body", error))?;
    let created = row
        .get::<_, String>(2)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "created", error))?
        .parse::<Timestamp>()
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "created", error))?;
    Memory::from_stored(seq, body, created)
        .map_err(|error| NtError::invalid_stored_memory_with_source(identity, "body", error))
}

pub(super) fn decode_segment(row: &Row<'_>) -> Result<MemorySegment> {
    let pk = row.get::<_, i64>(0).map_err(|error| {
        NtError::invalid_stored_memory_with_source("segment: unknown", "pk", error)
    })?;
    let identity = format!("segment row: {pk}");
    let level = row
        .get::<_, i64>(1)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "level", error))?;
    let block = row
        .get::<_, i64>(2)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "block", error))?;
    let node = decode_node(level, block)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "node", error))?;
    let summary = row
        .get::<_, String>(3)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "summary", error))?;
    let created = row
        .get::<_, String>(4)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "created", error))?
        .parse::<Timestamp>()
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "created", error))?;
    MemorySegment::from_stored(pk, node, summary, created)
        .map_err(|error| NtError::invalid_stored_memory_with_source(identity, "summary", error))
}

pub(super) fn node_range(node: SummaryNodeId) -> Result<RawRange> {
    range(node.level(), node.block()).ok_or_else(|| invalid_node(node, "has an invalid raw range"))
}

fn node_values(node: SummaryNodeId) -> Result<(i64, i64)> {
    let level = i64::try_from(node.level())
        .map_err(|_| invalid_node(node, "cannot be represented in SQLite"))?;
    let block = i64::try_from(node.block())
        .map_err(|_| invalid_node(node, "cannot be represented in SQLite"))?;
    Ok((level, block))
}

fn decode_node(level: i64, block: i64) -> Result<SummaryNodeId> {
    let level = u64::try_from(level).map_err(|_| invalid_node_value("invalid stored level"))?;
    let block = u64::try_from(block).map_err(|_| invalid_node_value("invalid stored block"))?;
    SummaryNodeId::new(level, block)
}

fn raw_bound(value: u64, node: SummaryNodeId) -> Result<i64> {
    i64::try_from(value).map_err(|_| invalid_node(node, "raw range exceeds SQLite identity"))
}

fn nonnegative_count(value: i64, name: &'static str) -> Result<u64> {
    u64::try_from(value).map_err(|_| NtError::InvalidValue {
        field: "memory status",
        value: format!("invalid {name}"),
    })
}

fn missing_memory(seq: i64) -> NtError {
    NtError::MemoryNotFound(seq)
}

fn invalid_node(node: SummaryNodeId, detail: &'static str) -> NtError {
    NtError::InvalidValue {
        field: "memory node",
        value: format!("{node} {detail}"),
    }
}

fn invalid_node_value(value: &'static str) -> NtError {
    NtError::InvalidValue {
        field: "memory node",
        value: value.to_string(),
    }
}
