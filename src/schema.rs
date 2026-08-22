use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
pub(crate) use crate::storage::InitOutcome;
use crate::storage::schema_engine::SchemaManifest;
use crate::storage::schema_engine::SchemaObject;
#[cfg(test)]
use crate::storage::schema_engine::{self, Identity};
use crate::storage::{self, OpenMode};

pub(crate) const APPLICATION_ID: i64 = 0x4e54_4e54;
pub(crate) const SCHEMA_VERSION: i64 = 3;

const SCHEMA_VERSION_OBJECT: SchemaObject = SchemaObject {
    object_type: "table",
    name: "schema_version",
    sql: "CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 3)
     ) WITHOUT ROWID",
};

const OBJECT_COUNT: usize =
    1 + crate::note::schema::OBJECTS.len() + crate::memory::schema::OBJECTS.len();

const fn schema_objects() -> [SchemaObject; OBJECT_COUNT] {
    let mut objects = [SCHEMA_VERSION_OBJECT; OBJECT_COUNT];
    let mut output = 1;
    let mut note = 0;
    while note < crate::note::schema::OBJECTS.len() {
        objects[output] = crate::note::schema::OBJECTS[note];
        output += 1;
        note += 1;
    }
    let mut memory = 0;
    while memory < crate::memory::schema::OBJECTS.len() {
        objects[output] = crate::memory::schema::OBJECTS[memory];
        output += 1;
        memory += 1;
    }
    objects
}

static OBJECTS: [SchemaObject; OBJECT_COUNT] = schema_objects();

const REQUIRED_SHADOW_TABLES: &[&str] = &[
    "note_fts_data",
    "note_fts_idx",
    "note_fts_docsize",
    "note_fts_config",
    "memory_fts_data",
    "memory_fts_idx",
    "memory_fts_docsize",
    "memory_fts_config",
    "memory_segment_fts_data",
    "memory_segment_fts_idx",
    "memory_segment_fts_docsize",
    "memory_segment_fts_config",
];

const ALLOWED_TRIGGERS: &[&str] = &[
    "notes_fts_insert",
    "notes_fts_update",
    "notes_fts_delete",
    "memories_fts_insert",
    "memories_fts_delete",
    "memories_immutable_identity",
    "memories_immutable_update",
    "memories_immutable_delete",
    "memory_segments_fts_insert",
    "memory_segments_fts_delete",
    "memory_segments_immutable_update",
    "memory_segments_immutable_identity",
];

pub(crate) static MANIFEST: SchemaManifest = SchemaManifest {
    application_id: APPLICATION_ID,
    version: SCHEMA_VERSION,
    objects: &OBJECTS,
    required_shadow_tables: REQUIRED_SHADOW_TABLES,
    allowed_triggers: ALLOWED_TRIGGERS,
    version_insert_sql: "INSERT INTO schema_version(singleton, version) VALUES (1, 3)",
};

pub(crate) fn initialize_at(path: &Path) -> Result<InitOutcome> {
    storage::initialize_at(path, &MANIFEST)
}

pub(crate) fn open_read_write(path: &Path) -> Result<Connection> {
    storage::open_at(path, OpenMode::ReadWrite, &MANIFEST)
}

pub(crate) fn open_read_only(path: &Path) -> Result<Connection> {
    storage::open_at(path, OpenMode::ReadOnly, &MANIFEST)
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
