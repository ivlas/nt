use crate::core::storage::schema_engine::{SchemaFragment, SchemaObject};

const OBJECTS: &[SchemaObject] = &[
    SchemaObject {
        object_type: "table",
        name: "note_library_refs",
        sql: "CREATE TABLE note_library_refs (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         library_item_pk INTEGER NOT NULL REFERENCES library_items(pk) ON DELETE CASCADE,
         PRIMARY KEY(note_pk, library_item_pk)
     )",
    },
    SchemaObject {
        object_type: "index",
        name: "note_library_refs_library_idx",
        sql: "CREATE INDEX note_library_refs_library_idx
         ON note_library_refs(library_item_pk, note_pk)",
    },
];

pub(crate) const NOTE_LIBRARY_SCHEMA: SchemaFragment = SchemaFragment {
    objects: OBJECTS,
    shadow_tables: &[],
    triggers: &[],
};
