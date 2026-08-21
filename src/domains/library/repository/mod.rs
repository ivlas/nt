use rusqlite::Connection;

mod query_sql;
mod store;

#[cfg(test)]
mod behavior_tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateItemOutcome {
    id: super::LibraryItemId,
    item_created: bool,
    capture_created: bool,
}

impl CreateItemOutcome {
    pub fn id(&self) -> &super::LibraryItemId {
        &self.id
    }
    pub fn item_created(&self) -> bool {
        self.item_created
    }
    pub fn capture_created(&self) -> bool {
        self.capture_created
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
