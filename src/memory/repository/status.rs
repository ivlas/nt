use super::Repository;
use super::stored::invalid_node_value;
use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryStatus {
    raw_count: u64,
    highest_seq: Option<i64>,
    summary_count: u64,
    pending_count: u64,
    highest_completed_level: Option<u64>,
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
}

impl Repository {
    pub(crate) fn status(&self) -> Result<MemoryStatus> {
        let values = self.connection.query_row(
            "SELECT
                 (SELECT COUNT(*) FROM memories),
                 (SELECT MAX(seq) FROM memories),
                 (SELECT COUNT(*) FROM memory_segments),
                 (SELECT COUNT(*) FROM memory_summary_jobs),
                 (SELECT MAX(level) FROM memory_segments)",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
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
        })
    }
}

fn nonnegative_count(value: i64, name: &'static str) -> Result<u64> {
    u64::try_from(value).map_err(|_| NtError::InvalidValue {
        field: "memory status",
        value: format!("invalid {name}"),
    })
}
