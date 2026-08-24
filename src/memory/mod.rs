mod model;
mod repository;
pub(crate) mod schema;
mod tree;

pub(crate) use model::{Memory, NewMemory, NewSummary, Summary};
pub(crate) use repository::{Repository, TreeItem};
pub(crate) use tree::{MemoryRange, WakeNode, next_summary, wake_cover};

pub(crate) const MEMORY_BODY_MAX_CHARS: usize = 512;
pub(crate) const WAKE_ENTRIES: usize = 128;
