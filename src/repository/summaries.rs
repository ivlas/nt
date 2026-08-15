use std::collections::BTreeSet;

use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::error::Result;
use crate::note::{CollectionPath, NoteId, Tag, Timestamp};
use crate::query::NoteQuery;

use super::Repository;
use super::query_sql::compile_query;
use super::stored::{decode_collection, decode_id, decode_tag, decode_timestamp};

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
            .prepare("SELECT DISTINCT tag FROM note_tags ORDER BY tag")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|value| decode_tag(&value?))
            .collect()
    }

    pub fn list_collections(&self) -> Result<Vec<CollectionPath>> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT collection FROM notes ORDER BY collection")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|value| decode_collection(&value?))
            .collect()
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
            "SELECT n.id, n.updated, n.collection, n.title,
                    COALESCE(
                        (SELECT json_group_array(tag)
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
            let stored_tags = row.get::<_, String>(4)?;
            let tags = serde_json::from_str::<Vec<String>>(&stored_tags)?
                .into_iter()
                .map(|tag| decode_tag(&tag))
                .collect::<Result<BTreeSet<_>>>()?;
            let outgoing = row.get::<_, i64>(5)?;
            visit(NoteSummary {
                id: decode_id(&row.get::<_, String>(0)?)?,
                updated: decode_timestamp(&row.get::<_, String>(1)?)?,
                collection: decode_collection(&row.get::<_, String>(2)?)?,
                title: row.get(3)?,
                tags,
                outgoing: u64::try_from(outgoing).expect("SQLite COUNT(*) results are nonnegative"),
            })?;
        }
        Ok(())
    }
}
