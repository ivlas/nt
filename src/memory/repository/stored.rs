use rusqlite::Row;

use crate::error::{NtError, Result};
use crate::memory::{Memory, MemorySegment, RawRange, SummaryNodeId, range};
use crate::note::Timestamp;

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

pub(super) fn node_values(node: SummaryNodeId) -> Result<(i64, i64)> {
    let level = i64::try_from(node.level())
        .map_err(|_| invalid_node(node, "cannot be represented in SQLite"))?;
    let block = i64::try_from(node.block())
        .map_err(|_| invalid_node(node, "cannot be represented in SQLite"))?;
    Ok((level, block))
}

pub(super) fn decode_node(level: i64, block: i64) -> Result<SummaryNodeId> {
    let level = u64::try_from(level).map_err(|_| invalid_node_value("invalid stored level"))?;
    let block = u64::try_from(block).map_err(|_| invalid_node_value("invalid stored block"))?;
    SummaryNodeId::new(level, block)
}

pub(super) fn raw_bound(value: u64, node: SummaryNodeId) -> Result<i64> {
    i64::try_from(value).map_err(|_| invalid_node(node, "raw range exceeds SQLite identity"))
}

pub(super) fn invalid_node(node: SummaryNodeId, detail: &'static str) -> NtError {
    NtError::InvalidValue {
        field: "memory node",
        value: format!("{node} {detail}"),
    }
}

pub(super) fn invalid_node_value(value: &'static str) -> NtError {
    NtError::InvalidValue {
        field: "memory node",
        value: value.to_string(),
    }
}
