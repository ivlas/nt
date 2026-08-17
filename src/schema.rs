use std::path::Path;

#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
use crate::core::storage::schema_engine::{self, Identity};
use crate::core::storage::schema_engine::{SchemaFragment, SchemaManifest, SchemaObject};
use crate::core::storage::{self, InitOutcome, OpenMode};
use crate::domains::note::Repository;
use crate::domains::note::schema::NOTE_SCHEMA;
use crate::error::Result;

pub(crate) const APPLICATION_ID: i64 = 0x4e54_4e54;
pub(crate) const SCHEMA_VERSION: i64 = 1;

const VERSION_OBJECT: SchemaObject = SchemaObject {
    object_type: "table",
    name: "schema_version",
    sql: "CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 1)
     ) WITHOUT ROWID",
};
const FRAGMENTS: &[SchemaFragment] = &[NOTE_SCHEMA];

pub(crate) static MANIFEST: SchemaManifest = SchemaManifest {
    application_id: APPLICATION_ID,
    version: SCHEMA_VERSION,
    version_object: VERSION_OBJECT,
    fragments: FRAGMENTS,
    version_insert_sql: "INSERT INTO schema_version(singleton, version) VALUES (1, 1)",
};

impl Repository {
    pub fn initialize_at(path: &Path) -> Result<InitOutcome> {
        storage::initialize_at(path, &MANIFEST)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        open_at(path, OpenMode::ReadWrite)
    }

    pub fn open_read_only(path: &Path) -> Result<Self> {
        open_at(path, OpenMode::ReadOnly)
    }
}

fn open_at(path: &Path, mode: OpenMode) -> Result<Repository> {
    storage::open_at(path, mode, &MANIFEST).map(Repository::from_connection)
}

#[cfg(test)]
pub(crate) fn initialize(connection: &mut Connection) -> Result<bool> {
    schema_engine::initialize(connection, &MANIFEST)
}

#[cfg(test)]
pub(crate) fn initialize_with(
    connection: &mut Connection,
    after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    schema_engine::initialize_with(connection, &MANIFEST, after_step)
}

#[cfg(test)]
pub(crate) fn inspect(connection: &Connection) -> Result<Identity> {
    schema_engine::inspect(connection, &MANIFEST)
}

#[cfg(test)]
mod tests;
