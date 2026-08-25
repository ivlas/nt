use std::collections::BTreeSet;

use rusqlite::params_from_iter;

use super::super::{Note, NoteId, NoteQuery, NoteRecord, Tag};
use crate::error::{NtError, Result, StoredNoteContext};

use super::Repository;
use super::query_sql::compile_ordered_query;
use super::stored::{
    decode_body_version, decode_collection, decode_id, decode_revision, decode_tag,
    decode_timestamp, stored_value,
};

impl Repository {
    pub fn visit_notes(
        &self,
        query: &NoteQuery,
        mut visit: impl FnMut(Note) -> Result<()>,
    ) -> Result<()> {
        let (query_sql, parameters) = compile_ordered_query(query);
        let sql = format!(
            "SELECT n.pk, n.id, n.collection, n.body, n.title, n.created, n.updated,
                    n.body_version, n.note_revision,
                    COALESCE(
                        (SELECT json_group_array(tag ORDER BY tag)
                         FROM note_tags read_tags WHERE read_tags.note_pk = n.pk),
                        '[]'
                    ),
                    COALESCE(
                        (SELECT json_group_array(target.id ORDER BY target.id)
                         FROM note_links read_links
                         JOIN notes target ON target.pk = read_links.target_note_pk
                         WHERE read_links.note_pk = n.pk),
                        '[]'
                    )
             FROM notes n {query_sql}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(parameters))?;
        while let Some(row) = rows.next()? {
            let row_id = row.get::<_, i64>(0)?;
            let row_context = StoredNoteContext::new(None, Some(row_id));
            let stored_id = stored_value::<String>(row, 1, &row_context, "id")?;
            let id = decode_id(&stored_id, &row_context)?;
            let context = StoredNoteContext::new(Some(id.to_string()), Some(row_id));
            let tags = decode_tags(&stored_value::<String>(row, 9, &context, "tag")?, &context)?;
            let links = decode_links(
                &stored_value::<String>(row, 10, &context, "links")?,
                &context,
            )?;
            visit(Note::rehydrate(
                NoteRecord {
                    id,
                    collection: decode_collection(
                        &stored_value::<String>(row, 2, &context, "collection")?,
                        &context,
                    )?,
                    body: stored_value(row, 3, &context, "body")?,
                    title: stored_value(row, 4, &context, "title")?,
                    created: decode_timestamp(
                        &stored_value::<String>(row, 5, &context, "created")?,
                        &context,
                        "created",
                    )?,
                    updated: decode_timestamp(
                        &stored_value::<String>(row, 6, &context, "updated")?,
                        &context,
                        "updated",
                    )?,
                    body_version: decode_body_version(
                        stored_value(row, 7, &context, "body_version")?,
                        &context,
                    )?,
                    revision: decode_revision(
                        stored_value(row, 8, &context, "note_revision")?,
                        &context,
                    )?,
                },
                tags,
                links,
            )?)?;
        }
        Ok(())
    }
}

fn decode_tags(value: &str, context: &StoredNoteContext) -> Result<BTreeSet<Tag>> {
    decode_set(value, context, "tag", |value| decode_tag(value, context))
}

fn decode_links(value: &str, context: &StoredNoteContext) -> Result<BTreeSet<NoteId>> {
    decode_set(value, context, "links", |value| decode_id(value, context))
}

fn decode_set<T: Ord>(
    value: &str,
    context: &StoredNoteContext,
    field: &'static str,
    decode: impl Fn(&str) -> Result<T>,
) -> Result<BTreeSet<T>> {
    serde_json::from_str::<Vec<String>>(value)
        .map_err(|error| NtError::invalid_stored_with_source(context.clone(), field, error))?
        .into_iter()
        .map(|value| decode(&value))
        .collect()
}
