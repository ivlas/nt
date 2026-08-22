use std::path::Path;

#[cfg(test)]
use rusqlite::Connection;

use crate::error::Result;
use crate::note::Repository;
use crate::note::schema::{ALLOWED_TRIGGERS, OBJECTS, REQUIRED_SHADOW_TABLES};
use crate::storage::schema_engine::SchemaManifest;
#[cfg(test)]
use crate::storage::schema_engine::{self, Identity};
use crate::storage::{self, InitOutcome, OpenMode};

pub(crate) const APPLICATION_ID: i64 = 0x4e54_4e54;
pub(crate) const SCHEMA_VERSION: i64 = 1;

pub(crate) static MANIFEST: SchemaManifest = SchemaManifest {
    application_id: APPLICATION_ID,
    version: SCHEMA_VERSION,
    objects: OBJECTS,
    required_shadow_tables: REQUIRED_SHADOW_TABLES,
    allowed_triggers: ALLOWED_TRIGGERS,
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
