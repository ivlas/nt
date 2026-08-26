use std::collections::BTreeSet;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::super::{CollectionPath, NewNote, Note, NoteId, NoteRecord, Tag, timestamp_now};
use crate::error::{NtError, Result, StoredNoteContext};

use super::Repository;
use super::changes::{ChangeOperation, record_change};
use super::stored::{
    decode_body_version, decode_collection, decode_id, decode_revision, decode_tag,
    decode_timestamp, stored_value,
};

impl Repository {
    pub fn create_note(&mut self, note: NewNote) -> Result<NoteId> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = NoteId::generate()?;
        note.validate_links_for(&id)?;
        let now = timestamp_now()?;
        let revision = next_revision(&transaction)?;
        transaction.execute(
            "INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![
                id.to_string(),
                note.collection().as_str(),
                note.body(),
                note.title(),
                now.as_str(),
                revision,
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
        record_change(&transaction, revision, &id, ChangeOperation::Add)?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn get_note(&self, id: &NoteId) -> Result<Note> {
        let transaction = self.connection.unchecked_transaction()?;
        let note = load_note(&transaction, id)?;
        transaction.commit()?;
        Ok(note)
    }

    pub fn delete_notes(&mut self, ids: &[NoteId]) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_unique(ids)?;
        if ids.is_empty() {
            transaction.commit()?;
            return Ok(());
        }
        let encoded_ids = encode_ids(ids);
        ensure_notes_exist(&transaction, ids, &encoded_ids)?;
        let affected_sources = select_ids(
            &transaction,
            "SELECT DISTINCT source.id
             FROM note_links links
             JOIN notes source ON source.pk = links.note_pk
             JOIN notes target ON target.pk = links.target_note_pk
             JOIN json_each(?1) requested ON requested.value = target.id
             WHERE source.id NOT IN (SELECT value FROM json_each(?1))
             ORDER BY source.id",
            &encoded_ids,
        )?;
        let revision = next_revision(&transaction)?;
        let updated = timestamp_now()?;
        if !affected_sources.is_empty() {
            let encoded_sources = encode_ids(&affected_sources);
            transaction.execute(
                "UPDATE notes SET updated = ?1, note_revision = ?2
                 WHERE id IN (SELECT value FROM json_each(?3))",
                params![updated.as_str(), revision, encoded_sources],
            )?;
            insert_changes(
                &transaction,
                revision,
                &encoded_sources,
                ChangeOperation::Metadata,
            )?;
        }
        insert_changes(
            &transaction,
            revision,
            &encoded_ids,
            ChangeOperation::Remove,
        )?;
        transaction.execute(
            "DELETE FROM notes WHERE id IN (SELECT value FROM json_each(?1))",
            [&encoded_ids],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn replace_body(
        &mut self,
        note: &Note,
        expected_version: u64,
        if_revision: Option<u64>,
    ) -> Result<()> {
        let expected_version = i64::try_from(expected_version)
            .map_err(|_| NtError::InvalidBodyVersion(expected_version))?;
        let body_version = i64::try_from(note.body_version())
            .map_err(|_| NtError::InvalidBodyVersion(note.body_version()))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_note_revision(&transaction, note.id(), if_revision)?;
        let revision = next_revision(&transaction)?;
        let changed = transaction.execute(
            "UPDATE notes
             SET body = ?1, title = ?2, updated = ?3, body_version = ?4, note_revision = ?5
             WHERE id = ?6 AND body_version = ?7",
            params![
                note.body(),
                note.title(),
                note.updated().as_str(),
                body_version,
                revision,
                note.id().to_string(),
                expected_version,
            ],
        )?;
        if changed == 0 {
            if stored_body_version(&transaction, note.id())?.is_none() {
                return Err(NtError::NoteNotFound(note.id().to_string()));
            }
            return Err(NtError::ConcurrentEdit(note.id().to_string()));
        }
        record_change(&transaction, revision, note.id(), ChangeOperation::Edit)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn verify_body_version(
        &mut self,
        id: &NoteId,
        expected_version: u64,
        if_revision: Option<u64>,
    ) -> Result<()> {
        if i64::try_from(expected_version).is_err() {
            return Err(NtError::InvalidBodyVersion(expected_version));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_note_revision(&transaction, id, if_revision)?;
        let actual_version = stored_body_version(&transaction, id)?
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
        if actual_version != expected_version {
            return Err(NtError::ConcurrentEdit(id.to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn move_note(
        &mut self,
        id: &NoteId,
        collection: &CollectionPath,
        if_revision: Option<u64>,
    ) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_note_exists(&transaction, id)?;
        ensure_note_revision(&transaction, id, if_revision)?;
        let updated = timestamp_now()?;
        let changed = transaction.execute(
            "UPDATE notes SET collection = ?1, updated = ?2
             WHERE id = ?3 AND collection <> ?1",
            params![collection.as_str(), updated.as_str(), id.to_string()],
        )? != 0;
        if changed {
            let revision = next_revision(&transaction)?;
            transaction.execute(
                "UPDATE notes SET note_revision = ?1 WHERE id = ?2",
                params![revision, id.to_string()],
            )?;
            record_change(&transaction, revision, id, ChangeOperation::Metadata)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn move_notes(&mut self, ids: &[NoteId], collection: &CollectionPath) -> Result<usize> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_unique(ids)?;
        let encoded_ids = encode_ids(ids);
        ensure_notes_exist(&transaction, ids, &encoded_ids)?;
        let changed_ids = select_ids_with_value(
            &transaction,
            "SELECT notes.id
             FROM notes
             JOIN json_each(?1) requested ON requested.value = notes.id
             WHERE notes.collection <> ?2
             ORDER BY notes.id",
            &encoded_ids,
            collection.as_str(),
        )?;
        if changed_ids.is_empty() {
            transaction.commit()?;
            return Ok(0);
        }
        let encoded_changed = encode_ids(&changed_ids);
        let revision = next_revision(&transaction)?;
        let updated = timestamp_now()?;
        transaction.execute(
            "UPDATE notes SET collection = ?1, updated = ?2, note_revision = ?3
             WHERE id IN (SELECT value FROM json_each(?4))",
            params![
                collection.as_str(),
                updated.as_str(),
                revision,
                encoded_changed
            ],
        )?;
        insert_changes(
            &transaction,
            revision,
            &encoded_changed,
            ChangeOperation::Metadata,
        )?;
        transaction.commit()?;
        Ok(changed_ids.len())
    }
}

pub(super) fn encode_ids(ids: &[NoteId]) -> String {
    serde_json::to_string(&ids.iter().map(ToString::to_string).collect::<Vec<String>>())
        .expect("note IDs serialize as JSON")
}

pub(super) fn ensure_unique(ids: &[NoteId]) -> Result<()> {
    let mut unique = BTreeSet::new();
    for id in ids {
        if !unique.insert(id) {
            return Err(NtError::DuplicateNoteId(id.to_string()));
        }
    }
    Ok(())
}

pub(super) fn ensure_notes_exist(
    transaction: &Transaction<'_>,
    ids: &[NoteId],
    encoded_ids: &str,
) -> Result<()> {
    let existing = select_ids(
        transaction,
        "SELECT notes.id
         FROM notes
         JOIN json_each(?1) requested ON requested.value = notes.id
         ORDER BY notes.id",
        encoded_ids,
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    for id in ids {
        if !existing.contains(id) {
            return Err(NtError::NoteNotFound(id.to_string()));
        }
    }
    Ok(())
}

pub(super) fn select_ids(
    transaction: &Transaction<'_>,
    sql: &str,
    encoded_ids: &str,
) -> Result<Vec<NoteId>> {
    let mut statement = transaction.prepare(sql)?;
    let values = statement
        .query_map([encoded_ids], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    values.into_iter().map(|value| value.parse()).collect()
}

pub(super) fn select_ids_with_value(
    transaction: &Transaction<'_>,
    sql: &str,
    encoded_ids: &str,
    value: &str,
) -> Result<Vec<NoteId>> {
    let mut statement = transaction.prepare(sql)?;
    let values = statement
        .query_map(params![encoded_ids, value], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    values.into_iter().map(|value| value.parse()).collect()
}

pub(super) fn insert_changes(
    transaction: &Transaction<'_>,
    revision: i64,
    encoded_ids: &str,
    operation: ChangeOperation,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO note_changes(revision, note_id, operation)
         SELECT ?1, value, ?2 FROM json_each(?3)",
        params![revision, operation.as_str(), encoded_ids],
    )?;
    Ok(())
}

pub(super) fn note_pk(transaction: &Transaction<'_>, id: &NoteId) -> Result<i64> {
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

pub(super) fn ensure_note_revision(
    transaction: &Transaction<'_>,
    id: &NoteId,
    expected: Option<u64>,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let mut statement = transaction.prepare("SELECT pk, note_revision FROM notes WHERE id = ?1")?;
    let mut rows = statement.query([id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Err(NtError::NoteNotFound(id.to_string()));
    };
    let row_id = row.get::<_, i64>(0)?;
    let context = StoredNoteContext::new(Some(id.to_string()), Some(row_id));
    let stored = stored_value::<i64>(row, 1, &context, "note_revision")?;
    let actual = decode_revision(stored, &context)?;
    if actual != expected {
        return Err(NtError::RevisionConflict {
            id: id.to_string(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn stored_body_version(connection: &rusqlite::Connection, id: &NoteId) -> Result<Option<u64>> {
    let mut statement = connection.prepare("SELECT pk, body_version FROM notes WHERE id = ?1")?;
    let mut rows = statement.query([id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let row_id = row.get::<_, i64>(0)?;
    let context = StoredNoteContext::new(Some(id.to_string()), Some(row_id));
    let version = decode_body_version(stored_value(row, 1, &context, "body_version")?, &context)?;
    Ok(Some(version))
}

pub(super) fn load_note(transaction: &Transaction<'_>, id: &NoteId) -> Result<Note> {
    let stored = {
        let mut statement = transaction.prepare(
            "SELECT pk, collection, body, title, created, updated, body_version, note_revision
             FROM notes WHERE id = ?1",
        )?;
        let mut rows = statement.query([id.to_string()])?;
        let Some(row) = rows.next()? else {
            return Err(NtError::NoteNotFound(id.to_string()));
        };
        let row_id = row.get::<_, i64>(0)?;
        let context = StoredNoteContext::new(Some(id.to_string()), Some(row_id));
        (
            row_id,
            context.clone(),
            stored_value::<String>(row, 1, &context, "collection")?,
            stored_value::<String>(row, 2, &context, "body")?,
            stored_value::<String>(row, 3, &context, "title")?,
            stored_value::<String>(row, 4, &context, "created")?,
            stored_value::<String>(row, 5, &context, "updated")?,
            stored_value::<i64>(row, 6, &context, "body_version")?,
            stored_value::<i64>(row, 7, &context, "note_revision")?,
        )
    };
    let tags = load_tags(transaction, stored.0, &stored.1)?;
    let links = load_links(transaction, stored.0)?;
    let body_version = decode_body_version(stored.7, &stored.1)?;
    Note::rehydrate(
        NoteRecord {
            id: id.clone(),
            collection: decode_collection(&stored.2, &stored.1)?,
            body: stored.3,
            title: stored.4,
            created: decode_timestamp(&stored.5, &stored.1, "created")?,
            updated: decode_timestamp(&stored.6, &stored.1, "updated")?,
            body_version,
            revision: decode_revision(stored.8, &stored.1)?,
        },
        tags,
        links,
    )
}

pub(super) fn next_revision(transaction: &Transaction<'_>) -> Result<i64> {
    transaction
        .query_row(
            "UPDATE global_revision SET revision = revision + 1
             WHERE singleton = 1 RETURNING revision",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn load_tags(
    connection: &rusqlite::Connection,
    note_pk: i64,
    context: &StoredNoteContext,
) -> Result<BTreeSet<Tag>> {
    let mut statement =
        connection.prepare("SELECT tag FROM note_tags WHERE note_pk = ?1 ORDER BY tag")?;
    let mut rows = statement.query([note_pk])?;
    let mut tags = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let value = stored_value::<String>(row, 0, context, "tag")?;
        tags.insert(decode_tag(&value, context)?);
    }
    Ok(tags)
}

fn load_links(connection: &rusqlite::Connection, note_pk: i64) -> Result<BTreeSet<NoteId>> {
    let mut statement = connection.prepare(
        "SELECT target.pk, target.id
         FROM note_links links
         JOIN notes target ON target.pk = links.target_note_pk
         WHERE links.note_pk = ?1
         ORDER BY target.id",
    )?;
    let mut rows = statement.query([note_pk])?;
    let mut links = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let target_row_id = row.get::<_, i64>(0)?;
        let context = StoredNoteContext::new(None, Some(target_row_id));
        let value = stored_value::<String>(row, 1, &context, "id")?;
        links.insert(decode_id(&value, &context)?);
    }
    Ok(links)
}
