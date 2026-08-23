use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::stored::{
    decode_memory, decode_node, decode_segment, invalid_node, node_range, node_values, raw_bound,
};
use super::{ExpansionItem, PendingJob, Repository};
use crate::error::{NtError, Result};
use crate::memory::{Children, MemorySegment, NewSummary, SummaryNodeId, children, parent, range};
use crate::note::timestamp_now;

impl Repository {
    pub(crate) fn get_summary(&self, node: SummaryNodeId) -> Result<MemorySegment> {
        let (level, block) = node_values(node)?;
        let mut statement = self.connection.prepare(
            "SELECT pk, level, block, summary, created FROM memory_segments
             WHERE level = ?1 AND block = ?2",
        )?;
        let mut rows = statement.query(params![level, block])?;
        let Some(row) = rows.next()? else {
            return Err(invalid_node(node, "summary not found"));
        };
        decode_segment(row)
    }

    #[cfg(test)]
    pub(crate) fn pending(&self, limit: Option<i64>) -> Result<Vec<PendingJob>> {
        let mut jobs = Vec::new();
        self.visit_pending(limit, |job| {
            jobs.push(job);
            Ok(())
        })?;
        Ok(jobs)
    }

    pub(crate) fn visit_pending(
        &self,
        limit: Option<i64>,
        mut visit: impl FnMut(PendingJob) -> Result<()>,
    ) -> Result<()> {
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
        while let Some(row) = rows.next()? {
            let node = decode_node(row.get(0)?, row.get(1)?)?;
            visit(PendingJob {
                node,
                raw_range: node_range(node)?,
            })?;
        }
        Ok(())
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
