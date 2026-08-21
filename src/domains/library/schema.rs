use crate::core::storage::schema_engine::{SchemaFragment, SchemaObject};

const OBJECTS: &[SchemaObject] = &[
    SchemaObject {
        object_type: "table",
        name: "library_items",
        sql: "CREATE TABLE library_items (
         pk INTEGER PRIMARY KEY,
         id TEXT NOT NULL UNIQUE,
         source TEXT NOT NULL UNIQUE,
         title TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         CHECK(length(id) = 36
               AND substr(id, 9, 1) = '-'
               AND substr(id, 14, 1) = '-'
               AND substr(id, 15, 1) = '7'
               AND substr(id, 19, 1) = '-'
               AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
               AND substr(id, 24, 1) = '-'
               AND length(replace(id, '-', '')) = 32
               AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
         CHECK(length(source) > 0),
         CHECK(length(title) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(updated GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "library_captures",
        sql: "CREATE TABLE library_captures (
         pk INTEGER PRIMARY KEY,
         item_pk INTEGER NOT NULL REFERENCES library_items(pk) ON DELETE CASCADE,
         captured TEXT NOT NULL,
         content TEXT NOT NULL,
         content_hash TEXT NOT NULL,
         UNIQUE(item_pk, content_hash),
         CHECK(captured GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(length(content) > 0),
         CHECK(length(content_hash) = 64
               AND content_hash NOT GLOB '*[^0-9a-f]*')
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "library_summaries",
        sql: "CREATE TABLE library_summaries (
         capture_pk INTEGER PRIMARY KEY REFERENCES library_captures(pk) ON DELETE CASCADE,
         summary TEXT NOT NULL,
         generator TEXT NOT NULL,
         version TEXT NOT NULL,
         created TEXT NOT NULL,
         CHECK(length(summary) > 0),
         CHECK(length(generator) > 0),
         CHECK(length(version) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     )",
    },
    SchemaObject {
        object_type: "table",
        name: "library_capture_fts",
        sql: "CREATE VIRTUAL TABLE library_capture_fts USING fts5(
         content,
         content = 'library_captures',
         content_rowid = 'pk',
         tokenize = 'unicode61 remove_diacritics 2'
     )",
    },
    SchemaObject {
        object_type: "trigger",
        name: "library_captures_fts_insert",
        sql: "CREATE TRIGGER library_captures_fts_insert AFTER INSERT ON library_captures BEGIN
         INSERT INTO library_capture_fts(rowid, content) VALUES (new.pk, new.content);
     END",
    },
    SchemaObject {
        object_type: "trigger",
        name: "library_captures_fts_delete",
        sql: "CREATE TRIGGER library_captures_fts_delete BEFORE DELETE ON library_captures BEGIN
         INSERT INTO library_capture_fts(library_capture_fts, rowid, content)
             VALUES ('delete', old.pk, old.content);
     END",
    },
    SchemaObject {
        object_type: "trigger",
        name: "library_captures_immutable_update",
        sql: "CREATE TRIGGER library_captures_immutable_update BEFORE UPDATE ON library_captures BEGIN
         SELECT RAISE(ABORT, 'library captures are immutable');
     END",
    },
    SchemaObject {
        object_type: "trigger",
        name: "library_captures_immutable_delete",
        sql: "CREATE TRIGGER library_captures_immutable_delete BEFORE DELETE ON library_captures
     WHEN EXISTS (SELECT 1 FROM library_items WHERE pk = old.item_pk) BEGIN
         SELECT RAISE(ABORT, 'library captures are immutable');
     END",
    },
    SchemaObject {
        object_type: "index",
        name: "library_items_updated_idx",
        sql: "CREATE INDEX library_items_updated_idx ON library_items(updated DESC, id DESC)",
    },
    SchemaObject {
        object_type: "index",
        name: "library_captures_latest_idx",
        sql: "CREATE INDEX library_captures_latest_idx
         ON library_captures(item_pk, captured DESC, pk DESC)",
    },
];

pub(crate) const LIBRARY_SCHEMA: SchemaFragment = SchemaFragment {
    objects: OBJECTS,
    shadow_tables: &[
        "library_capture_fts_data",
        "library_capture_fts_idx",
        "library_capture_fts_docsize",
        "library_capture_fts_config",
    ],
    triggers: &[
        "library_captures_fts_insert",
        "library_captures_fts_delete",
        "library_captures_immutable_update",
        "library_captures_immutable_delete",
    ],
};
