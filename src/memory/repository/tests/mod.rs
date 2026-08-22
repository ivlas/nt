use std::collections::BTreeSet;

use rusqlite::Connection;

use super::{
    ContextItem, ExpansionItem, Repository,
    context::{lexical_raw_candidates, lexical_summary_candidates},
    context_output_char_count,
};
use crate::error::NtError;
use crate::memory::schema::OBJECTS;
use crate::memory::tree::span;
use crate::memory::{
    MEMORY_CONTEXT_CHARS, MemoryContextQuery, MemoryListQuery, MemoryRecallQuery, NewMemory,
    NewSummary, SummaryNodeId, range,
};

mod context;
mod raw;
mod scale;
mod status;
mod summaries;

fn repository() -> Repository {
    let connection = Connection::open_in_memory().unwrap();
    install_schema(&connection);
    Repository::from_connection(connection)
}

fn install_schema(connection: &Connection) {
    for object in OBJECTS {
        connection.execute_batch(object.sql).unwrap();
    }
}

fn append(repository: &mut Repository, count: usize, prefix: &str) {
    for index in 0..count {
        repository
            .append(NewMemory::new(format!("{prefix} {index}")).unwrap())
            .unwrap();
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn node(level: u64, block: u64) -> SummaryNodeId {
    SummaryNodeId::new(level, block).unwrap()
}
