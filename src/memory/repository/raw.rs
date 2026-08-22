use rusqlite::{TransactionBehavior, params};

use super::Repository;
use super::stored::{decode_memory, invalid_node_value, node_range, node_values, raw_bound};
use crate::error::{NtError, Result};
use crate::memory::{Memory, MemoryListQuery, MemoryRecallQuery, NewMemory, level0_for_seq};
use crate::note::timestamp_now;

impl Repository {
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
            let raw_range = node_range(node)?;
            let raw_start = raw_bound(raw_range.start(), node)?;
            let raw_end = raw_bound(raw_range.end(), node)?;
            let (level, block) = node_values(node)?;
            transaction.execute(
                "INSERT INTO memory_summary_jobs(level, block)
                 SELECT ?1, ?2
                 WHERE (SELECT COUNT(*) FROM memories WHERE seq BETWEEN ?3 AND ?4) = 16
                   AND NOT EXISTS (
                      SELECT 1 FROM memory_segments WHERE level = ?1 AND block = ?2
                   )
                 ON CONFLICT(level, block) DO NOTHING",
                params![level, block, raw_start, raw_end],
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
            return Err(NtError::MemoryNotFound(seq));
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
        sql.push_str(" ORDER BY m.seq ASC");
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
}
