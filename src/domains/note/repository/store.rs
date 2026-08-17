use std::collections::BTreeSet;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::super::{CollectionPath, NewNote, Note, NoteId, NoteRecord, Tag, timestamp_now};
use crate::error::{NtError, Result, StoredNoteContext};

use super::Repository;
use super::stored::{
    decode_body_version, decode_collection, decode_id, decode_tag, decode_timestamp, stored_value,
};

impl Repository {
    pub fn create_note(&mut self, note: NewNote) -> Result<NoteId> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = NoteId::generate()?;
        note.validate_links_for(&id)?;
        let now = timestamp_now()?;
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
        let mut pks = Vec::with_capacity(ids.len());
        let mut unique = BTreeSet::new();
        for id in ids {
            if !unique.insert(id) {
                return Err(NtError::DuplicateNoteId(id.to_string()));
            }
        }
        for id in ids {
            pks.push(note_pk(&transaction, id)?);
        }
        let updated = timestamp_now()?;
        for pk in &pks {
            transaction.execute(
                "UPDATE notes SET updated = ?1
                 WHERE pk IN (
                     SELECT note_pk FROM note_links WHERE target_note_pk = ?2
                 )",
                params![updated.as_str(), pk],
            )?;
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
            if stored_body_version(&transaction, note.id())?.is_none() {
                return Err(NtError::NoteNotFound(note.id().to_string()));
            }
            return Err(NtError::ConcurrentEdit(note.id().to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn verify_body_version(&self, id: &NoteId, expected_version: u64) -> Result<()> {
        if i64::try_from(expected_version).is_err() {
            return Err(NtError::InvalidBodyVersion(expected_version));
        }
        let actual_version = stored_body_version(&self.connection, id)?
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
        if actual_version != expected_version {
            return Err(NtError::ConcurrentEdit(id.to_string()));
        }
        Ok(())
    }

    pub fn move_note(&mut self, id: &NoteId, collection: &CollectionPath) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_note_exists(&transaction, id)?;
        let updated = timestamp_now()?;
        let changed = transaction.execute(
            "UPDATE notes SET collection = ?1, updated = ?2
             WHERE id = ?3 AND collection <> ?1",
            params![collection.as_str(), updated.as_str(), id.to_string()],
        )? != 0;
        transaction.commit()?;
        Ok(changed)
    }
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
            "SELECT pk, collection, body, title, created, updated, body_version
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
        },
        tags,
        links,
    )
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
