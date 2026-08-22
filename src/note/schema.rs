use crate::storage::schema_engine::SchemaObject;

pub(crate) const OBJECTS: &[SchemaObject] = &[
    SchemaObject {
        object_type: "table",
        name: "schema_version",
        sql: "CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 1)
     ) WITHOUT ROWID",
    },
    SchemaObject {
        object_type: "table",
        name: "notes",
        sql: "CREATE TABLE notes (
         pk INTEGER PRIMARY KEY,
         id TEXT NOT NULL UNIQUE,
         collection TEXT NOT NULL,
         body TEXT NOT NULL,
         title TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         body_version INTEGER NOT NULL DEFAULT 1,
         CHECK(length(id) = 36
               AND substr(id, 9, 1) = '-'
               AND substr(id, 14, 1) = '-'
               AND substr(id, 15, 1) = '7'
               AND substr(id, 19, 1) = '-'
               AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
               AND substr(id, 24, 1) = '-'
               AND length(replace(id, '-', '')) = 32
               AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
         CHECK(length(collection) > 0
               AND collection NOT GLOB '*[^a-z0-9_/-]*'
               AND substr(collection, 1, 1) <> '/'
               AND substr(collection, -1, 1) <> '/'
               AND instr(collection, '//') = 0),
         CHECK(length(body) > 0),
         CHECK(length(title) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(updated GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(body_version > 0)
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "note_tags",
        sql: "CREATE TABLE note_tags (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         tag TEXT NOT NULL,
         PRIMARY KEY(note_pk, tag),
         CHECK(length(tag) > 0 AND tag NOT GLOB '*[^a-z0-9_-]*')
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "note_links",
        sql: "CREATE TABLE note_links (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         target_note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         PRIMARY KEY(note_pk, target_note_pk),
         CHECK(note_pk <> target_note_pk)
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "note_fts",
        sql: "CREATE VIRTUAL TABLE note_fts USING fts5(
         title,
         body,
         content = 'notes',
         content_rowid = 'pk',
         tokenize = 'unicode61 remove_diacritics 2'
     )",
    },
    SchemaObject {
        object_type: "trigger",
        name: "notes_fts_insert",
        sql: "CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END",
    },
    SchemaObject {
        object_type: "trigger",
        name: "notes_fts_update",
        sql: "CREATE TRIGGER notes_fts_update AFTER UPDATE OF title, body ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END",
    },
    SchemaObject {
        object_type: "trigger",
        name: "notes_fts_delete",
        sql: "CREATE TRIGGER notes_fts_delete BEFORE DELETE ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
     END",
    },
    SchemaObject {
        object_type: "index",
        name: "notes_created_idx",
        sql: "CREATE INDEX notes_created_idx ON notes(created DESC, id DESC)",
    },
    SchemaObject {
        object_type: "index",
        name: "notes_updated_idx",
        sql: "CREATE INDEX notes_updated_idx ON notes(updated DESC, id DESC)",
    },
    SchemaObject {
        object_type: "index",
        name: "notes_collection_updated_idx",
        sql: "CREATE INDEX notes_collection_updated_idx
         ON notes(collection, updated DESC, id DESC)",
    },
    SchemaObject {
        object_type: "index",
        name: "note_tags_tag_note_idx",
        sql: "CREATE INDEX note_tags_tag_note_idx ON note_tags(tag, note_pk)",
    },
    SchemaObject {
        object_type: "index",
        name: "note_links_target_idx",
        sql: "CREATE INDEX note_links_target_idx ON note_links(target_note_pk)",
    },
];

pub(crate) const REQUIRED_SHADOW_TABLES: &[&str] = &[
    "note_fts_data",
    "note_fts_idx",
    "note_fts_docsize",
    "note_fts_config",
];

pub(crate) const ALLOWED_TRIGGERS: &[&str] =
    &["notes_fts_insert", "notes_fts_update", "notes_fts_delete"];
