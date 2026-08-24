use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::error::{NtError, Result};
use crate::note::timestamp_now;

use super::{Memory, MemoryRange, NewMemory, NewSummary, Summary, WakeNode};

#[derive(Debug)]
pub(crate) enum TreeItem {
    Raw(Memory),
    Summary(Summary),
}

pub(crate) struct Repository {
    connection: Connection,
}

impl Repository {
    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }

    pub(crate) fn append_memory(&mut self, memory: NewMemory) -> Result<u64> {
        let created = timestamp_now()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM memory",
            [],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO memory(sequence, created_at, body) VALUES (?1, ?2, ?3)",
            params![sequence, created.as_str(), memory.body()],
        )?;
        transaction.commit()?;
        u64::try_from(sequence).map_err(|_| NtError::InvalidValue {
            field: "memory sequence",
            value: sequence.to_string(),
        })
    }

    pub(crate) fn raw_count(&self) -> Result<u64> {
        let (count, maximum): (i64, Option<i64>) =
            self.connection
                .query_row("SELECT COUNT(*), MAX(sequence) FROM memory", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
        let count = u64::try_from(count).map_err(|error| {
            NtError::invalid_stored_memory_with_source("history", "count", error)
        })?;
        let expected_max = count
            .checked_sub(1)
            .and_then(|value| i64::try_from(value).ok());
        if maximum != expected_max {
            return Err(NtError::InvalidStoredMemory {
                identity: "history".to_string(),
                field: "sequence",
                source: None,
            });
        }
        Ok(count)
    }

    pub(crate) fn get_raw(&self, sequence: u64) -> Result<Memory> {
        let sequence = sqlite_integer(sequence, "memory sequence")?;
        let mut statement = self
            .connection
            .prepare("SELECT sequence, created_at, body FROM memory WHERE sequence = ?1")?;
        let mut rows = statement.query([sequence])?;
        let Some(row) = rows.next()? else {
            return Err(NtError::MemoryNotFound(sequence));
        };
        decode_memory(row)
    }

    pub(crate) fn recall(
        &self,
        pattern: &str,
        mut visit: impl FnMut(&Memory) -> Result<()>,
    ) -> Result<()> {
        let mut statement = self.connection.prepare(
            "SELECT sequence, created_at, body FROM memory
             WHERE instr(body, ?1) > 0 ORDER BY sequence",
        )?;
        let mut rows = statement.query([pattern])?;
        while let Some(row) = rows.next()? {
            visit(&decode_memory(row)?)?;
        }
        Ok(())
    }

    pub(crate) fn get_summary(&self, range: MemoryRange) -> Result<Option<Summary>> {
        let (lo, hi) = sqlite_range(range)?;
        let mut statement = self
            .connection
            .prepare("SELECT lo, hi, body FROM memory_summary WHERE lo = ?1 AND hi = ?2")?;
        let mut rows = statement.query([lo, hi])?;
        rows.next()?.map(decode_summary).transpose()
    }

    pub(crate) fn next_summary(&self) -> Result<Option<MemoryRange>> {
        let raw_count = self.raw_count()?;
        let mut size = 2_u64;
        while size <= raw_count {
            let limit = raw_count - raw_count % size;
            let sqlite_size = sqlite_integer(size, "memory range")?;
            let maximum = self
                .connection
                .query_row(
                    "SELECT MAX(lo) FROM memory_summary WHERE hi - lo = ?1",
                    [sqlite_size],
                    |row| row.get::<_, Option<i64>>(0),
                )?
                .map(|lo| {
                    u64::try_from(lo).map_err(|error| {
                        NtError::invalid_stored_memory_with_source(
                            format!("summary {lo}"),
                            "lo",
                            error,
                        )
                    })
                })
                .transpose()?;

            match maximum {
                None => return Ok(Some(MemoryRange::from_parts(0, size))),
                Some(lo) if lo + size < limit => {
                    return Ok(Some(MemoryRange::from_parts(lo + size, lo + size * 2)));
                }
                Some(_) => {
                    if let Some(lo) = self.first_summary_gap(size, limit)? {
                        return Ok(Some(MemoryRange::from_parts(lo, lo + size)));
                    }
                }
            }
            let Some(next) = size.checked_mul(2) else {
                break;
            };
            size = next;
        }
        Ok(None)
    }

    pub(crate) fn summary_inputs(&self, range: MemoryRange) -> Result<Vec<TreeItem>> {
        let count = self.raw_count()?;
        if range.hi() > count {
            return Err(invalid_range(range, "extends past raw history"));
        }
        match range.children() {
            (WakeNode::Raw(lo), WakeNode::Raw(hi)) => Ok(vec![
                TreeItem::Raw(self.get_raw(lo)?),
                TreeItem::Raw(self.get_raw(hi)?),
            ]),
            (WakeNode::Summary(left), WakeNode::Summary(right)) => Ok(vec![
                TreeItem::Summary(
                    self.get_summary(left)?
                        .ok_or_else(|| invalid_range(left, "summary not found"))?,
                ),
                TreeItem::Summary(
                    self.get_summary(right)?
                        .ok_or_else(|| invalid_range(right, "summary not found"))?,
                ),
            ]),
            _ => unreachable!(),
        }
    }

    pub(crate) fn put_summary(&mut self, range: MemoryRange, summary: NewSummary) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = raw_count_in(&transaction)?;
        if range.hi() > count || !children_exist(&transaction, range)? {
            return Err(invalid_range(range, "is not buildable"));
        }
        let (lo, hi) = sqlite_range(range)?;
        let existing = transaction
            .query_row(
                "SELECT body FROM memory_summary WHERE lo = ?1 AND hi = ?2",
                [lo, hi],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match existing {
            Some(body) if body == summary.body() => {}
            Some(_) => return Err(invalid_range(range, "conflicts with existing summary")),
            None => {
                transaction.execute(
                    "INSERT INTO memory_summary(lo, hi, body) VALUES (?1, ?2, ?3)",
                    params![lo, hi, summary.body()],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn zoom(&self, range: MemoryRange) -> Result<Vec<TreeItem>> {
        if self.get_summary(range)?.is_none() {
            return Err(invalid_range(range, "summary not found"));
        }
        self.summary_inputs(range)
    }

    pub(crate) fn forget(&mut self, range: MemoryRange) -> Result<usize> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (lo, hi) = sqlite_range(range)?;
        let removed = transaction.execute(
            "DELETE FROM memory_summary WHERE lo <= ?1 AND hi >= ?2",
            [lo, hi],
        )?;
        if removed == 0 {
            return Err(invalid_range(range, "summary not found"));
        }
        transaction.commit()?;
        Ok(removed)
    }

    pub(crate) fn wake_items(&self, cover: &[WakeNode]) -> Result<Vec<TreeItem>> {
        cover
            .iter()
            .map(|node| match *node {
                WakeNode::Raw(sequence) => self.get_raw(sequence).map(TreeItem::Raw),
                WakeNode::Summary(range) => self
                    .get_summary(range)?
                    .map(TreeItem::Summary)
                    .ok_or_else(|| invalid_range(range, "summary missing; run nt memory nap")),
            })
            .collect()
    }

    fn first_summary_gap(&self, size: u64, limit: u64) -> Result<Option<u64>> {
        let sqlite_size = sqlite_integer(size, "memory range")?;
        let mut statement = self
            .connection
            .prepare("SELECT lo FROM memory_summary WHERE hi - lo = ?1 ORDER BY lo")?;
        let mut rows = statement.query([sqlite_size])?;
        let mut expected = 0_u64;
        while let Some(row) = rows.next()? {
            let lo = row.get::<_, i64>(0).map_err(|error| {
                NtError::invalid_stored_memory_with_source("summary: unknown", "lo", error)
            })?;
            let lo = u64::try_from(lo).map_err(|error| {
                NtError::invalid_stored_memory_with_source(format!("summary {lo}"), "lo", error)
            })?;
            if lo != expected {
                return Ok(Some(expected));
            }
            expected += size;
        }
        Ok((expected < limit).then_some(expected))
    }
}

fn raw_count_in(transaction: &rusqlite::Transaction<'_>) -> Result<u64> {
    let count = transaction.query_row("SELECT COUNT(*) FROM memory", [], |row| {
        row.get::<_, i64>(0)
    })?;
    u64::try_from(count)
        .map_err(|error| NtError::invalid_stored_memory_with_source("history", "count", error))
}

fn children_exist(transaction: &rusqlite::Transaction<'_>, range: MemoryRange) -> Result<bool> {
    match range.children() {
        (WakeNode::Raw(_), WakeNode::Raw(_)) => Ok(true),
        (WakeNode::Summary(left), WakeNode::Summary(right)) => {
            Ok(summary_exists(transaction, left)? && summary_exists(transaction, right)?)
        }
        _ => unreachable!(),
    }
}

fn summary_exists(transaction: &rusqlite::Transaction<'_>, range: MemoryRange) -> Result<bool> {
    let (lo, hi) = sqlite_range(range)?;
    transaction
        .query_row(
            "SELECT 1 FROM memory_summary WHERE lo = ?1 AND hi = ?2",
            [lo, hi],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(Into::into)
}

fn decode_memory(row: &rusqlite::Row<'_>) -> Result<Memory> {
    let sequence = row.get::<_, i64>(0).map_err(|error| {
        NtError::invalid_stored_memory_with_source("identity: unknown", "sequence", error)
    })?;
    let identity = format!("#{sequence}");
    let created = row.get::<_, String>(1).map_err(|error| {
        NtError::invalid_stored_memory_with_source(&identity, "created_at", error)
    })?;
    let body = row
        .get::<_, String>(2)
        .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "body", error))?;
    Memory::from_stored(sequence, created, body)
}

fn decode_summary(row: &rusqlite::Row<'_>) -> Result<Summary> {
    let lo = row.get::<_, i64>(0).map_err(|error| {
        NtError::invalid_stored_memory_with_source("summary: unknown", "lo", error)
    })?;
    let hi = row.get::<_, i64>(1).map_err(|error| {
        NtError::invalid_stored_memory_with_source(format!("summary {lo}"), "hi", error)
    })?;
    let body = row.get::<_, String>(2).map_err(|error| {
        NtError::invalid_stored_memory_with_source(format!("summary {lo}-{hi}"), "body", error)
    })?;
    Summary::from_stored(lo, hi, body)
}

fn sqlite_integer(value: u64, field: &'static str) -> Result<i64> {
    i64::try_from(value).map_err(|_| NtError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

fn sqlite_range(range: MemoryRange) -> Result<(i64, i64)> {
    Ok((
        sqlite_integer(range.lo(), "memory range")?,
        sqlite_integer(range.hi(), "memory range")?,
    ))
}

fn invalid_range(range: MemoryRange, detail: &'static str) -> NtError {
    NtError::InvalidValue {
        field: "memory range",
        value: format!("{range} {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{Repository, TreeItem, decode_summary};
    use crate::memory::schema::OBJECTS;
    use crate::memory::{NewMemory, NewSummary};

    fn repository() -> Repository {
        let connection = Connection::open_in_memory().unwrap();
        for object in OBJECTS {
            connection.execute_batch(object.sql).unwrap();
        }
        Repository::from_connection(connection)
    }

    fn append(repository: &mut Repository, count: u64) {
        for sequence in 0..count {
            assert_eq!(
                repository
                    .append_memory(NewMemory::new(format!("event {sequence}")).unwrap())
                    .unwrap(),
                sequence
            );
        }
    }

    #[test]
    fn raw_history_is_zero_based_ordered_and_recallable_without_summaries() {
        let mut repository = repository();
        append(&mut repository, 4);
        assert_eq!(repository.raw_count().unwrap(), 4);
        let mut sequences = Vec::new();
        repository
            .recall("event", |memory| {
                sequences.push(memory.seq());
                Ok(())
            })
            .unwrap();
        assert_eq!(sequences, [0, 1, 2, 3]);
        assert_eq!(repository.get_raw(2).unwrap().body(), "event 2");
    }

    #[test]
    fn malformed_stored_values_are_rejected_without_normalization() {
        let repository = repository();
        repository
            .connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 INSERT INTO memory(sequence, created_at, body)
                 VALUES (0, '2026-08-22T12:34:56Z', 'event' || char(10));
                 INSERT INTO memory_summary(lo, hi, body) VALUES (1, 3, 'misaligned');
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .unwrap();

        assert!(repository.get_raw(0).is_err());
        let mut statement = repository
            .connection
            .prepare("SELECT lo, hi, body FROM memory_summary")
            .unwrap();
        let mut rows = statement.query([]).unwrap();
        assert!(decode_summary(rows.next().unwrap().unwrap()).is_err());
    }

    #[test]
    fn summaries_build_zoom_forget_and_reappear_as_work() {
        let mut repository = repository();
        append(&mut repository, 8);
        for (range, body) in [
            ("0-1", "zero one"),
            ("2-3", "two three"),
            ("4-5", "four five"),
            ("6-7", "six seven"),
            ("0-3", "zero through three"),
            ("4-7", "four through seven"),
            ("0-7", "all events"),
        ] {
            assert_eq!(
                repository.next_summary().unwrap(),
                Some(range.parse().unwrap())
            );
            repository
                .put_summary(range.parse().unwrap(), NewSummary::new(body).unwrap())
                .unwrap();
        }
        assert_eq!(repository.next_summary().unwrap(), None);

        let children = repository.zoom("0-7".parse().unwrap()).unwrap();
        assert!(
            children
                .iter()
                .all(|item| matches!(item, TreeItem::Summary(_)))
        );
        let leaves = repository.zoom("0-1".parse().unwrap()).unwrap();
        assert!(leaves.iter().all(|item| matches!(item, TreeItem::Raw(_))));

        assert_eq!(repository.forget("0-1".parse().unwrap()).unwrap(), 3);
        assert_eq!(repository.raw_count().unwrap(), 8);
        assert_eq!(repository.get_raw(0).unwrap().body(), "event 0");
        assert_eq!(
            repository.next_summary().unwrap(),
            Some("0-1".parse().unwrap())
        );
    }
}
