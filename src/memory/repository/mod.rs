use rusqlite::Connection;

use super::{Memory, MemorySegment, RawRange, SummaryNodeId};

mod context;
mod raw;
mod status;
mod stored;
mod summaries;
#[cfg(test)]
mod tests;

pub(crate) use context::ContextItem;
#[cfg(test)]
pub(crate) use context::context_output_char_count;
pub(crate) use status::MemoryStatus;

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

pub(crate) struct Repository {
    connection: Connection,
}

impl Repository {
    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }
}
