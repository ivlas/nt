use std::fmt;

use rusqlite::Connection;

mod changes;
mod query_sql;
mod reads;
mod relationships;
mod store;
mod stored;
mod summaries;
#[cfg(test)]
mod tests;

pub use changes::Change;
#[cfg(test)]
pub use changes::ChangeOperation;
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
