use std::fmt;

use rusqlite::Connection;

#[cfg(test)]
mod behavior_tests;
mod query_sql;
mod relationships;
mod store;
mod stored;
mod summaries;

pub use summaries::NoteSummary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddOrRemove<T> {
    Add(T),
    Remove(T),
}

impl<T: fmt::Display> fmt::Display for AddOrRemove<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add(value) => write!(formatter, "+{value}"),
            Self::Remove(value) => write!(formatter, "-{value}"),
        }
    }
}

pub struct Repository {
    pub(crate) connection: Connection,
}

impl Repository {
    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }
}
