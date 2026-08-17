use std::collections::BTreeSet;

use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::super::{CollectionPath, NoteId, NoteQuery, Tag, Timestamp};
use crate::error::{NtError, Result, StoredNoteContext};

use super::Repository;
use super::query_sql::compile_query;
use super::stored::{decode_collection, decode_id, decode_tag, decode_timestamp, stored_value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteSummary {
    id: NoteId,
    updated: Timestamp,
    collection: CollectionPath,
    title: String,
    tags: BTreeSet<Tag>,
    outgoing: u64,
}

impl NoteSummary {
    pub fn id(&self) -> &NoteId {
        &self.id
    }

    pub fn updated(&self) -> &Timestamp {
        &self.updated
    }

    pub fn collection(&self) -> &CollectionPath {
        &self.collection
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }

    pub fn outgoing(&self) -> u64 {
        self.outgoing
    }
}

impl Repository {
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut statement = self
            .connection
            .prepare("SELECT MIN(note_pk), tag FROM note_tags GROUP BY tag ORDER BY tag")?;
        let mut rows = statement.query([])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            let unknown = StoredNoteContext::new(None, None);
            let row_id = stored_value::<i64>(row, 0, &unknown, "tag")?;
            let context = StoredNoteContext::new(None, Some(row_id));
            let value = stored_value::<String>(row, 1, &context, "tag")?;
            tags.push(decode_tag(&value, &context)?);
        }
        Ok(tags)
    }

    pub fn list_collections(&self) -> Result<Vec<CollectionPath>> {
        let mut statement = self.connection.prepare(
            "SELECT MIN(pk), collection FROM notes GROUP BY collection ORDER BY collection",
        )?;
        let mut rows = statement.query([])?;
        let mut collections = Vec::new();
        while let Some(row) = rows.next()? {
            let row_id = row.get::<_, i64>(0)?;
            let context = StoredNoteContext::new(None, Some(row_id));
            let value = stored_value::<String>(row, 1, &context, "collection")?;
            collections.push(decode_collection(&value, &context)?);
        }
        Ok(collections)
    }

    pub fn visit_note_summaries(
        &self,
        query: &NoteQuery,
        mut visit: impl FnMut(NoteSummary) -> Result<()>,
    ) -> Result<()> {
        let (where_sql, mut parameters) = compile_query(query);
        let limit_sql = if let Some(limit) = query.limit() {
            parameters.push(Value::Integer(limit));
            format!("LIMIT ?{}", parameters.len())
        } else {
            String::new()
        };
        let sql = format!(
            "SELECT n.pk, n.id, n.updated, n.collection, n.title,
                    COALESCE(
                        (SELECT json_group_array(
                                    CASE WHEN typeof(tag) = 'text' THEN tag END
                                )
                         FROM note_tags summary_tags
                         WHERE summary_tags.note_pk = n.pk),
                        '[]'
                    ),
                    (SELECT COUNT(*) FROM note_links links WHERE links.note_pk = n.pk)
             FROM notes n {where_sql}
             ORDER BY n.updated DESC, n.id DESC
             {limit_sql}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(parameters))?;
        while let Some(row) = rows.next()? {
            let row_id = row.get::<_, i64>(0)?;
            let row_context = StoredNoteContext::new(None, Some(row_id));
            let stored_id = stored_value::<String>(row, 1, &row_context, "id")?;
            let id = decode_id(&stored_id, &row_context)?;
            let context = StoredNoteContext::new(Some(id.to_string()), Some(row_id));
            let stored_tags = stored_value::<String>(row, 5, &context, "tag")?;
            let tags = serde_json::from_str::<Vec<Option<String>>>(&stored_tags)
                .map_err(|error| {
                    NtError::invalid_stored_with_source(context.clone(), "tag", error)
                })?
                .into_iter()
                .map(|tag| {
                    let tag = tag.ok_or_else(|| NtError::invalid_stored(context.clone(), "tag"))?;
                    decode_tag(&tag, &context)
                })
                .collect::<Result<BTreeSet<_>>>()?;
            let outgoing = stored_value::<i64>(row, 6, &context, "links")?;
            visit(NoteSummary {
                id,
                updated: decode_timestamp(
                    &stored_value::<String>(row, 2, &context, "updated")?,
                    &context,
                    "updated",
                )?,
                collection: decode_collection(
                    &stored_value::<String>(row, 3, &context, "collection")?,
                    &context,
                )?,
                title: stored_value(row, 4, &context, "title")?,
                tags,
                outgoing: u64::try_from(outgoing).expect("SQLite COUNT(*) results are nonnegative"),
            })?;
        }
        Ok(())
    }
}
