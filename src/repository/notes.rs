use std::collections::BTreeSet;

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter};

use crate::error::{NtError, Result};
use crate::note::{CollectionPath, NewNote, Note, NoteId, Tag, Timestamp, timestamp_now};
use crate::query::{Filter, NoteQuery};

use super::{AddOrRemove, Repository};

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
    pub fn create_note(&mut self, note: NewNote) -> Result<NoteId> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = NoteId::generate();
        note.validate_links_for(&id)?;
        let now = timestamp_now();
        transaction.execute(
            "INSERT INTO notes(id, collection, body, title, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                id.to_string(),
                note.collection().as_str(),
                note.body(),
                note.title(),
                now.as_str(),
            ],
        )?;
        let source_pk = transaction.last_insert_rowid();
        for tag in note.tags() {
            transaction.execute(
                "INSERT INTO note_tags(note_pk, tag) VALUES (?1, ?2)",
                params![source_pk, tag.as_str()],
            )?;
        }
        for target in note.links() {
            let target_pk = note_pk(&transaction, target)?;
            transaction.execute(
                "INSERT INTO note_links(note_pk, target_note_pk) VALUES (?1, ?2)",
                params![source_pk, target_pk],
            )?;
        }
        transaction.commit()?;
        Ok(id)
    }

    pub fn get_note(&mut self, id: &NoteId) -> Result<Note> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let note = load_note(&transaction, id)?;
        transaction.commit()?;
        Ok(note)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT tag FROM note_tags ORDER BY tag")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|value| value?.parse())
            .collect()
    }

    pub fn list_collections(&self) -> Result<Vec<CollectionPath>> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT collection FROM notes ORDER BY collection")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|value| value?.parse())
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
                .map(|tag| tag.parse())
                .collect::<Result<BTreeSet<_>>>()?;
            let outgoing = row.get::<_, i64>(5)?;
            visit(NoteSummary {
                id: row.get::<_, String>(0)?.parse()?,
                updated: row.get::<_, String>(1)?.parse()?,
                collection: row.get::<_, String>(2)?.parse()?,
                title: row.get(3)?,
                tags,
                outgoing: u64::try_from(outgoing).expect("SQLite COUNT(*) results are nonnegative"),
            })?;
        }
        Ok(())
    }

    pub fn delete_notes(&mut self, ids: &[NoteId]) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut pks = Vec::with_capacity(ids.len());
        for id in ids {
            pks.push(note_pk(&transaction, id)?);
        }
        for pk in pks {
            transaction.execute("DELETE FROM notes WHERE pk = ?1", [pk])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn replace_body(&mut self, note: &Note, expected_version: u64) -> Result<()> {
        let expected_version = i64::try_from(expected_version)
            .map_err(|_| NtError::InvalidBodyVersion(expected_version))?;
        let body_version = i64::try_from(note.body_version())
            .map_err(|_| NtError::InvalidBodyVersion(note.body_version()))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE notes
             SET body = ?1, title = ?2, updated = ?3, body_version = ?4
             WHERE id = ?5 AND body_version = ?6",
            params![
                note.body(),
                note.title(),
                note.updated().as_str(),
                body_version,
                note.id().to_string(),
                expected_version,
            ],
        )?;
        if changed == 0 {
            if note_exists(&transaction, note.id())? {
                return Err(NtError::ConcurrentEdit(note.id().to_string()));
            }
            return Err(NtError::NoteNotFound(note.id().to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn verify_body_version(&mut self, id: &NoteId, expected_version: u64) -> Result<()> {
        let expected_version = i64::try_from(expected_version)
            .map_err(|_| NtError::InvalidBodyVersion(expected_version))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actual_version: i64 = transaction
            .query_row(
                "SELECT body_version FROM notes WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
        if actual_version != expected_version {
            return Err(NtError::ConcurrentEdit(id.to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn move_note(&mut self, id: &NoteId, collection: &CollectionPath) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_note_exists(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE notes SET collection = ?1, updated = ?2
             WHERE id = ?3 AND collection <> ?1",
            params![
                collection.as_str(),
                timestamp_now().as_str(),
                id.to_string()
            ],
        )? != 0;
        transaction.commit()?;
        Ok(changed)
    }

    pub fn change_tag(&mut self, id: &NoteId, operation: AddOrRemove<Tag>) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pk = note_pk(&transaction, id)?;
        let changed = match operation {
            AddOrRemove::Add(tag) => transaction.execute(
                "INSERT OR IGNORE INTO note_tags(note_pk, tag) VALUES (?1, ?2)",
                params![pk, tag.as_str()],
            )?,
            AddOrRemove::Remove(tag) => transaction.execute(
                "DELETE FROM note_tags WHERE note_pk = ?1 AND tag = ?2",
                params![pk, tag.as_str()],
            )?,
        } != 0;
        touch_if_changed(&transaction, id, changed)?;
        transaction.commit()?;
        Ok(changed)
    }

    pub fn change_link(&mut self, id: &NoteId, operation: AddOrRemove<NoteId>) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source_pk = note_pk(&transaction, id)?;
        let target = match &operation {
            AddOrRemove::Add(target) | AddOrRemove::Remove(target) => target,
        };
        if target == id {
            return Err(NtError::SelfLink);
        }
        let changed = match &operation {
            AddOrRemove::Add(target) => {
                let target_pk = note_pk(&transaction, target)?;
                transaction.execute(
                    "INSERT OR IGNORE INTO note_links(note_pk, target_note_pk) VALUES (?1, ?2)",
                    params![source_pk, target_pk],
                )?
            }
            AddOrRemove::Remove(target) => transaction.execute(
                "DELETE FROM note_links
                 WHERE note_pk = ?1 AND target_note_pk =
                      (SELECT pk FROM notes WHERE id = ?2)",
                params![source_pk, target.to_string()],
            )?,
        } != 0;
        if changed {
            let updated = timestamp_now();
            transaction.execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![updated.as_str(), id.to_string(), target.to_string()],
            )?;
        }
        transaction.commit()?;
        Ok(changed)
    }
}

fn note_pk(transaction: &Transaction<'_>, id: &NoteId) -> Result<i64> {
    transaction
        .query_row(
            "SELECT pk FROM notes WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))
}

fn note_exists(transaction: &Transaction<'_>, id: &NoteId) -> Result<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)",
            [id.to_string()],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn ensure_note_exists(transaction: &Transaction<'_>, id: &NoteId) -> Result<()> {
    if note_exists(transaction, id)? {
        Ok(())
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

fn touch_if_changed(transaction: &Transaction<'_>, id: &NoteId, changed: bool) -> Result<()> {
    if changed {
        transaction.execute(
            "UPDATE notes SET updated = ?1 WHERE id = ?2",
            params![timestamp_now().as_str(), id.to_string()],
        )?;
    }
    Ok(())
}

fn load_note(transaction: &Transaction<'_>, id: &NoteId) -> Result<Note> {
    let stored = transaction
        .query_row(
            "SELECT pk, collection, body, title, created, updated, body_version
             FROM notes WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    let tags = load_tags(transaction, stored.0)?;
    let links = load_links(transaction, stored.0)?;
    let body_version = u64::try_from(stored.6)
        .map_err(|_| NtError::InvalidBodyVersion(stored.6.cast_unsigned()))?;
    Note::rehydrate(
        id.clone(),
        stored.1.parse()?,
        stored.2,
        stored.3,
        stored.4.parse()?,
        stored.5.parse()?,
        body_version,
        tags,
        links,
    )
}

fn load_tags(connection: &rusqlite::Connection, note_pk: i64) -> Result<BTreeSet<Tag>> {
    let mut statement =
        connection.prepare("SELECT tag FROM note_tags WHERE note_pk = ?1 ORDER BY tag")?;
    statement
        .query_map([note_pk], |row| row.get::<_, String>(0))?
        .map(|value| value?.parse())
        .collect()
}

fn load_links(connection: &rusqlite::Connection, note_pk: i64) -> Result<BTreeSet<NoteId>> {
    let mut statement = connection.prepare(
        "SELECT target.id
         FROM note_links links
         JOIN notes target ON target.pk = links.target_note_pk
         WHERE links.note_pk = ?1
         ORDER BY target.id",
    )?;
    statement
        .query_map([note_pk], |row| row.get::<_, String>(0))?
        .map(|value| value?.parse())
        .collect()
}

fn compile_query(query: &NoteQuery) -> (String, Vec<Value>) {
    if query.filters().is_empty() && query.lexical_terms().is_empty() {
        return (String::new(), Vec::new());
    }
    let mut parameters = Vec::new();
    let mut expressions = query
        .filters()
        .iter()
        .map(|filter| compile_filter(filter, &mut parameters))
        .collect::<Vec<_>>();
    if !query.lexical_terms().is_empty() {
        let fts_query = query
            .lexical_terms()
            .iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" AND ");
        let parameter = push_parameter(&mut parameters, &fts_query);
        expressions.push(format!(
            "n.pk IN (SELECT rowid FROM note_fts WHERE note_fts MATCH ?{parameter})"
        ));
    }
    (format!("WHERE {}", expressions.join(" AND ")), parameters)
}

fn compile_filter(filter: &Filter, parameters: &mut Vec<Value>) -> String {
    match filter {
        Filter::IdPrefix(prefix) => {
            let parameter = push_parameter(parameters, prefix);
            format!("substr(n.id, 1, length(?{parameter})) = ?{parameter}")
        }
        Filter::Collection(collection) => {
            let parameter = push_parameter(parameters, collection.as_str());
            format!("n.collection = ?{parameter}")
        }
        Filter::Tag(tag) => {
            let parameter = push_parameter(parameters, tag.as_str());
            format!(
                "EXISTS (SELECT 1 FROM note_tags filter_tags
                 WHERE filter_tags.note_pk = n.pk AND filter_tags.tag = ?{parameter})"
            )
        }
        Filter::LinksTo(target) => {
            let parameter = push_parameter(parameters, &target.to_string());
            format!(
                "EXISTS (SELECT 1 FROM note_links filter_links
                 JOIN notes filter_target ON filter_target.pk = filter_links.target_note_pk
                 WHERE filter_links.note_pk = n.pk AND filter_target.id = ?{parameter})"
            )
        }
        Filter::LinkedFrom(source) => {
            let parameter = push_parameter(parameters, &source.to_string());
            format!(
                "n.pk IN (SELECT filter_links.target_note_pk
                 FROM notes filter_source
                 JOIN note_links filter_links ON filter_links.note_pk = filter_source.pk
                 WHERE filter_source.id = ?{parameter})"
            )
        }
        Filter::CreatedSince(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("n.created >= ?{parameter}")
        }
        Filter::UpdatedSince(timestamp) => {
            let parameter = push_parameter(parameters, timestamp.as_str());
            format!("n.updated >= ?{parameter}")
        }
        Filter::Not(inner) => format!("NOT ({})", compile_filter(inner, parameters)),
    }
}

fn push_parameter(parameters: &mut Vec<Value>, value: &str) -> usize {
    parameters.push(Value::Text(value.to_string()));
    parameters.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{initialize_at, open_at, schema};

    fn repository() -> Repository {
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        schema::initialize(&mut connection).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .unwrap();
        Repository { connection }
    }

    fn summaries(repository: &Repository, query: &NoteQuery) -> Vec<NoteSummary> {
        let mut summaries = Vec::new();
        repository
            .visit_note_summaries(query, |summary| {
                summaries.push(summary);
                Ok(())
            })
            .unwrap();
        summaries
    }

    #[test]
    fn creates_loads_lists_and_deletes_notes() {
        let mut repository = repository();
        let id = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Storage\nBody")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        let note = repository.get_note(&id).unwrap();
        assert_eq!(note.body(), "# Storage\nBody");

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id(), &id);
        assert_eq!(notes[0].tags().len(), 1);
        repository.delete_notes(std::slice::from_ref(&id)).unwrap();
        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::NoteNotFound(_))
        ));
    }

    #[test]
    fn complete_note_load_uses_one_read_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        initialize_at(&path).unwrap();
        let mut writer = open_at(&path).unwrap();
        let target = writer
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = writer
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Source")
                    .unwrap()
                    .with_tags(["old".parse().unwrap()])
                    .with_links([target.clone()]),
            )
            .unwrap();
        let mut reader = open_at(&path).unwrap();
        let expected = reader.get_note(&source).unwrap();

        let transaction = reader
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .unwrap();
        transaction
            .query_row(
                "SELECT 1 FROM notes WHERE id = ?1",
                [source.to_string()],
                |_| Ok(()),
            )
            .unwrap();
        writer
            .change_tag(&source, AddOrRemove::Remove("old".parse().unwrap()))
            .unwrap();
        writer
            .change_tag(&source, AddOrRemove::Add("new".parse().unwrap()))
            .unwrap();
        writer.delete_notes(std::slice::from_ref(&target)).unwrap();

        assert_eq!(load_note(&transaction, &source).unwrap(), expected);
        transaction.commit().unwrap();
        assert_ne!(reader.get_note(&source).unwrap(), expected);
    }

    #[test]
    fn list_and_find_load_all_summary_tags() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# First\nbatched")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap(), "sqlite".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Second\nbatched")
                    .unwrap()
                    .with_tags(["cli".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Untagged\nbatched").unwrap())
            .unwrap();

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(notes.len(), 3);
        let first = notes.iter().find(|note| note.title() == "First").unwrap();
        assert_eq!(
            first.tags().iter().map(Tag::as_str).collect::<Vec<_>>(),
            ["rust", "sqlite"]
        );
        let untagged = notes
            .iter()
            .find(|note| note.title() == "Untagged")
            .unwrap();
        assert!(untagged.tags().is_empty());

        let query = NoteQuery::parse_find(&["batched".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query).len(), 3);
    }

    #[test]
    fn list_and_find_are_complete_by_default() {
        let mut repository = repository();
        for index in 0..1101 {
            repository
                .create_note(
                    NewNote::new(
                        CollectionPath::inbox(),
                        format!("# Note {index}\nshared limit term"),
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        let list = NoteQuery::parse_list(&[]).unwrap();
        assert_eq!(summaries(&repository, &list).len(), 1101);
        let find = NoteQuery::parse_find(&["shared".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &find).len(), 1101);

        let list = NoteQuery::parse_list(&["limit:7".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &list).len(), 7);
        let find = NoteQuery::parse_find(&["shared".to_string(), "limit:5".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &find).len(), 5);
    }

    #[test]
    fn summary_visiting_stops_before_later_rows_are_decoded() {
        let mut repository = repository();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Valid").unwrap())
            .unwrap();
        repository
            .connection
            .execute(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                 VALUES ('malformed', 'inbox', '# Invalid', 'Invalid',
                         '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z')",
                [],
            )
            .unwrap();

        let mut visited = 0;
        let result = repository.visit_note_summaries(&NoteQuery::default(), |_| {
            visited += 1;
            Err(NtError::Message("stop visiting".to_string()))
        });

        assert!(matches!(result, Err(NtError::Message(_))));
        assert_eq!(visited, 1);
    }

    #[test]
    fn lists_current_tags_and_collections_once_in_lexical_order() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new("work/nt".parse().unwrap(), "# Work")
                    .unwrap()
                    .with_tags(["sqlite".parse().unwrap(), "rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Inbox")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();

        assert_eq!(
            repository
                .list_tags()
                .unwrap()
                .iter()
                .map(Tag::as_str)
                .collect::<Vec<_>>(),
            ["rust", "sqlite"]
        );
        assert_eq!(
            repository
                .list_collections()
                .unwrap()
                .iter()
                .map(CollectionPath::as_str)
                .collect::<Vec<_>>(),
            ["inbox", "work/nt"]
        );
    }

    #[test]
    fn summaries_count_outgoing_links() {
        let mut repository = repository();
        let first_target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First target").unwrap())
            .unwrap();
        let second_target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Second target").unwrap())
            .unwrap();
        let source = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Linked source")
                    .unwrap()
                    .with_links([first_target, second_target]),
            )
            .unwrap();

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(
            notes
                .iter()
                .find(|note| note.id() == &source)
                .unwrap()
                .outgoing(),
            2
        );
        assert!(
            notes
                .iter()
                .filter(|note| note.id() != &source)
                .all(|note| note.outgoing() == 0)
        );

        let query = NoteQuery::parse_find(&["linked source".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query)[0].outgoing(), 2);
    }

    #[test]
    fn validates_link_targets_and_atomic_deletion() {
        let mut repository = repository();
        let missing: NoteId = "018fbe0a-6c00-7000-8000-000000000001".parse().unwrap();
        let result = repository.create_note(
            NewNote::new(CollectionPath::inbox(), "# Link")
                .unwrap()
                .with_links([missing.clone()]),
        );
        assert!(matches!(result, Err(NtError::NoteNotFound(_))));

        let first = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First").unwrap())
            .unwrap();
        let result = repository.delete_notes(&[first.clone(), missing]);
        assert!(matches!(result, Err(NtError::NoteNotFound(_))));
        assert!(repository.get_note(&first).is_ok());
    }

    #[test]
    fn list_filters_are_and_combined_and_negatable() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new("work/nt".parse().unwrap(), "# Rust")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Other").unwrap())
            .unwrap();
        let query = NoteQuery::parse_list(&[
            "collection:work/nt".to_string(),
            "not:tag:sqlite".to_string(),
        ])
        .unwrap();
        let notes = summaries(&repository, &query);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "Rust");
    }

    #[test]
    fn directional_link_filters_compose_and_preserve_order_and_limits() {
        let mut repository = repository();
        let b = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# B\nsqlite target")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        let c = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# C\nother target").unwrap())
            .unwrap();
        let a = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# A\nsource")
                    .unwrap()
                    .with_links([b.clone(), c.clone()]),
            )
            .unwrap();
        let d = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# D\nsource")
                    .unwrap()
                    .with_links([b.clone()]),
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-01T00:00:00Z' WHERE id IN (?1, ?2)",
                params![b.to_string(), c.to_string()],
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
                params![a.to_string(), d.to_string()],
            )
            .unwrap();

        let ids = |query: NoteQuery| {
            summaries(&repository, &query)
                .into_iter()
                .map(|summary| summary.id().clone())
                .collect::<Vec<_>>()
        };
        let mut expected_sources = vec![a.clone(), d.clone()];
        expected_sources.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
        let mut expected_targets = vec![b.clone(), c.clone()];
        expected_targets.sort_by_key(|id| std::cmp::Reverse(id.to_string()));

        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("links-to:{b}")]).unwrap()),
            expected_sources
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{a}")]).unwrap()),
            expected_targets
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{d}")]).unwrap()),
            std::slice::from_ref(&b)
        );
        assert!(ids(NoteQuery::parse_list(&[format!("linked-from:{c}")]).unwrap()).is_empty());

        let tagged =
            NoteQuery::parse_list(&[format!("linked-from:{a}"), "tag:rust".to_string()]).unwrap();
        assert_eq!(ids(tagged), std::slice::from_ref(&b));

        let found =
            NoteQuery::parse_find(&["sqlite".to_string(), format!("linked-from:{a}")]).unwrap();
        assert_eq!(ids(found), std::slice::from_ref(&b));

        let excluded = NoteQuery::parse_list(&[format!("not:linked-from:{a}")]).unwrap();
        assert_eq!(
            ids(excluded).into_iter().collect::<BTreeSet<_>>(),
            [a.clone(), d.clone()].into_iter().collect()
        );

        let limited =
            NoteQuery::parse_list(&[format!("linked-from:{a}"), "limit:1".to_string()]).unwrap();
        assert_eq!(ids(limited), expected_targets[..1]);

        let missing = "018fbe0a-6c00-7000-8000-000000000001";
        assert!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{missing}")]).unwrap()).is_empty()
        );
    }

    #[test]
    fn body_updates_detect_conflicts_but_metadata_does_not_create_them() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Original").unwrap())
            .unwrap();
        let mut note = repository.get_note(&id).unwrap();
        let expected = note.body_version();
        repository
            .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
            .unwrap();
        note.replace_body("# Edited", "2026-05-28T15:00:00Z".parse().unwrap())
            .unwrap();
        repository.replace_body(&note, expected).unwrap();
        assert_eq!(repository.get_note(&id).unwrap().body_version(), 2);

        let mut stale = repository.get_note(&id).unwrap();
        let stale_version = stale.body_version();
        repository
            .connection
            .execute(
                "UPDATE notes SET body_version = body_version + 1 WHERE id = ?1",
                [id.to_string()],
            )
            .unwrap();
        stale
            .replace_body("# Stale", "2026-05-28T16:00:00Z".parse().unwrap())
            .unwrap();
        assert!(matches!(
            repository.replace_body(&stale, stale_version),
            Err(NtError::ConcurrentEdit(_))
        ));
        assert!(matches!(
            repository.verify_body_version(&id, stale_version),
            Err(NtError::ConcurrentEdit(_))
        ));
    }

    #[test]
    fn metadata_changes_are_idempotent_and_touch_only_real_changes() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-01T00:00:00Z' WHERE id = ?1",
                [id.to_string()],
            )
            .unwrap();

        assert!(
            repository
                .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
                .unwrap()
        );
        let updated = repository.get_note(&id).unwrap().updated().clone();
        assert!(
            !repository
                .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
                .unwrap()
        );
        assert_eq!(repository.get_note(&id).unwrap().updated(), &updated);
        assert!(
            !repository
                .change_tag(&id, AddOrRemove::Remove("missing".parse().unwrap()))
                .unwrap()
        );
        assert!(
            repository
                .change_link(&id, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        assert!(
            !repository
                .change_link(&id, AddOrRemove::Add(target))
                .unwrap()
        );
        assert!(
            repository
                .move_note(&id, &"work/nt".parse().unwrap())
                .unwrap()
        );
        assert!(
            !repository
                .move_note(&id, &"work/nt".parse().unwrap())
                .unwrap()
        );
        let query = NoteQuery::parse_list(&["collection:work/nt".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query).len(), 1);
    }

    #[test]
    fn link_changes_touch_both_endpoints_only_when_the_edge_changes() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        let old = "2026-01-01T00:00:00Z";
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![old, source.to_string(), target.to_string()],
            )
            .unwrap();

        assert!(
            repository
                .change_link(&source, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        let source_updated = repository.get_note(&source).unwrap().updated().clone();
        let target_updated = repository.get_note(&target).unwrap().updated().clone();
        assert_eq!(source_updated, target_updated);
        assert_ne!(source_updated.as_str(), old);

        assert!(
            !repository
                .change_link(&source, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        assert_eq!(
            repository.get_note(&source).unwrap().updated(),
            &source_updated
        );
        assert_eq!(
            repository.get_note(&target).unwrap().updated(),
            &target_updated
        );

        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![old, source.to_string(), target.to_string()],
            )
            .unwrap();
        assert!(
            repository
                .change_link(&source, AddOrRemove::Remove(target.clone()))
                .unwrap()
        );
        let source_updated = repository.get_note(&source).unwrap().updated().clone();
        let target_updated = repository.get_note(&target).unwrap().updated().clone();
        assert_eq!(source_updated, target_updated);
        assert_ne!(source_updated.as_str(), old);

        assert!(
            !repository
                .change_link(&source, AddOrRemove::Remove(target.clone()))
                .unwrap()
        );
        assert_eq!(
            repository.get_note(&source).unwrap().updated(),
            &source_updated
        );
        assert_eq!(
            repository.get_note(&target).unwrap().updated(),
            &target_updated
        );
    }

    #[test]
    fn links_reject_self_references() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        assert!(matches!(
            repository.change_link(&id, AddOrRemove::Add(id.clone())),
            Err(NtError::SelfLink)
        ));
    }

    #[test]
    fn removing_a_link_to_a_deleted_target_is_idempotent() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Source")
                    .unwrap()
                    .with_links([target.clone()]),
            )
            .unwrap();
        repository
            .delete_notes(std::slice::from_ref(&target))
            .unwrap();
        assert!(
            !repository
                .change_link(&source, AddOrRemove::Remove(target))
                .unwrap()
        );
    }

    #[test]
    fn find_uses_literal_complete_tokens_with_structured_filters() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new(
                    "work/nt".parse().unwrap(),
                    "# Café storage\nOwnership and borrowing.",
                )
                .unwrap()
                .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Storage shed").unwrap())
            .unwrap();

        let query =
            NoteQuery::parse_find(&["cafe ownership".to_string(), "tag:rust".to_string()]).unwrap();
        let notes = summaries(&repository, &query);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "Café storage");

        let prefix = NoteQuery::parse_find(&["stor".to_string()]).unwrap();
        assert!(summaries(&repository, &prefix).is_empty());
        let punctuation = NoteQuery::parse_find(&["(storage*)".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &punctuation).len(), 2);
    }

    #[test]
    fn list_find_and_explicit_limits_preserve_deterministic_ordering() {
        let mut repository = repository();
        let first = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First\nordered").unwrap())
            .unwrap();
        let second = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Second\nordered").unwrap())
            .unwrap();
        let newest = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Newest\nordered").unwrap())
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
                params![first.to_string(), second.to_string()],
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-03T00:00:00Z' WHERE id = ?1",
                [newest.to_string()],
            )
            .unwrap();

        let mut tied = [first, second];
        tied.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
        let expected = vec![newest, tied[0].clone(), tied[1].clone()];
        for query in [
            NoteQuery::parse_list(&[]).unwrap(),
            NoteQuery::parse_find(&["ordered".to_string()]).unwrap(),
        ] {
            let actual = summaries(&repository, &query)
                .into_iter()
                .map(|summary| summary.id().clone())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }

        let limited =
            NoteQuery::parse_find(&["ordered".to_string(), "limit:2".to_string()]).unwrap();
        let actual = summaries(&repository, &limited)
            .into_iter()
            .map(|summary| summary.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected[..2]);
    }
}
