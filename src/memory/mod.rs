mod model;
pub(crate) mod query;
mod repository;
pub(crate) mod schema;
mod tree;

pub(crate) use model::{Memory, MemorySegment, NewMemory, NewSummary, SummaryNodeId};
pub(crate) use query::{MemoryContextQuery, MemoryListQuery, MemoryRecallQuery};
pub(crate) use repository::{ContextItem, ExpansionItem, PendingJob, Repository};
pub(crate) use tree::{Children, RawRange, children, level0_for_seq, parent, range};

pub(crate) const MEMORY_ENTRY_MAX_CHARS: usize = 1_024;
pub(crate) const MEMORY_SUMMARY_MAX_CHARS: usize = 1_024;
pub(crate) const MEMORY_CONTEXT_CHARS: usize = 32_768;
pub(crate) const MEMORY_FANOUT: u64 = 16;
