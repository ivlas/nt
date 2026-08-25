use std::path::Path;

use rusqlite::Connection;
use rusqlite::types::Value;

use crate::error::Result;
pub(crate) use crate::storage::InitOutcome;
use crate::storage::schema_engine::SchemaManifest;
use crate::storage::schema_engine::SchemaObject;
#[cfg(test)]
use crate::storage::schema_engine::{self, Identity};
use crate::storage::{self, OpenMode};

pub(crate) const APPLICATION_ID: i64 = 0x4e54_4e54;
pub(crate) const SCHEMA_VERSION: i64 = 5;

const SCHEMA_VERSION_OBJECT: SchemaObject = SchemaObject {
    object_type: "table",
    name: "schema_version",
    sql: "CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 5)
     ) WITHOUT ROWID",
};

const OBJECT_COUNT: usize = 1 + crate::note::schema::OBJECTS.len();

const fn schema_objects() -> [SchemaObject; OBJECT_COUNT] {
    let mut objects = [SCHEMA_VERSION_OBJECT; OBJECT_COUNT];
    let mut output = 1;
    let mut note = 0;
    while note < crate::note::schema::OBJECTS.len() {
        objects[output] = crate::note::schema::OBJECTS[note];
        output += 1;
        note += 1;
    }
    objects
}

static OBJECTS: [SchemaObject; OBJECT_COUNT] = schema_objects();

const REQUIRED_SHADOW_TABLES: &[&str] = &[
    "note_fts_data",
    "note_fts_idx",
    "note_fts_docsize",
    "note_fts_config",
];

const ALLOWED_TRIGGERS: &[&str] = &["notes_fts_insert", "notes_fts_update", "notes_fts_delete"];

pub(crate) static MANIFEST: SchemaManifest = SchemaManifest {
    application_id: APPLICATION_ID,
    version: SCHEMA_VERSION,
    objects: &OBJECTS,
    required_shadow_tables: REQUIRED_SHADOW_TABLES,
    allowed_triggers: ALLOWED_TRIGGERS,
    version_insert_sql: "INSERT INTO schema_version(singleton, version) VALUES (1, 5);
                         INSERT INTO global_revision(singleton, revision) VALUES (1, 0)",
};

pub(crate) fn initialize_at(path: &Path) -> Result<InitOutcome> {
    storage::initialize_at(path, &MANIFEST)
}

pub(crate) fn open_read_write(path: &Path) -> Result<Connection> {
    let connection = storage::open_at(path, OpenMode::ReadWrite, &MANIFEST)?;
    validate_global_revision(&connection)?;
    Ok(connection)
}

pub(crate) fn open_read_only(path: &Path) -> Result<Connection> {
    let connection = storage::open_at(path, OpenMode::ReadOnly, &MANIFEST)?;
    validate_global_revision(&connection)?;
    Ok(connection)
}

fn validate_global_revision(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("SELECT revision FROM global_revision")?;
    let revisions = statement
        .query_map([], |row| row.get::<_, Value>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match revisions.as_slice() {
        [Value::Integer(revision)] if *revision >= 0 => Ok(()),
        _ => Err(crate::error::NtError::NotNtDatabase),
    }
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
