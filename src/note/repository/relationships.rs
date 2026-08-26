use rusqlite::{Transaction, TransactionBehavior, params};

use super::super::{NoteId, Tag, timestamp_now};
use crate::error::{NtError, Result};

use super::changes::{ChangeOperation, record_change};
use super::store::{
    encode_ids, ensure_note_revision, ensure_notes_exist, ensure_unique, insert_changes,
    next_revision, note_pk, select_ids_with_value,
};
use super::{AddOrRemove, Repository};

impl Repository {
    pub fn change_tag(
        &mut self,
        id: &NoteId,
        operation: AddOrRemove<Tag>,
        if_revision: Option<u64>,
    ) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pk = note_pk(&transaction, id)?;
        ensure_note_revision(&transaction, id, if_revision)?;
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

    pub fn change_tags(&mut self, ids: &[NoteId], operation: AddOrRemove<Tag>) -> Result<usize> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_unique(ids)?;
        let encoded_ids = encode_ids(ids);
        ensure_notes_exist(&transaction, ids, &encoded_ids)?;
        let (tag, query) = match &operation {
            AddOrRemove::Add(tag) => (
                tag,
                "SELECT notes.id
                 FROM notes
                 JOIN json_each(?1) requested ON requested.value = notes.id
                 LEFT JOIN note_tags tags ON tags.note_pk = notes.pk AND tags.tag = ?2
                 WHERE tags.note_pk IS NULL
                 ORDER BY notes.id",
            ),
            AddOrRemove::Remove(tag) => (
                tag,
                "SELECT notes.id
                 FROM notes
                 JOIN json_each(?1) requested ON requested.value = notes.id
                 JOIN note_tags tags ON tags.note_pk = notes.pk AND tags.tag = ?2
                 ORDER BY notes.id",
            ),
        };
        let changed_ids = select_ids_with_value(&transaction, query, &encoded_ids, tag.as_str())?;
        if changed_ids.is_empty() {
            transaction.commit()?;
            return Ok(0);
        }
        let encoded_changed = encode_ids(&changed_ids);
        match operation {
            AddOrRemove::Add(tag) => transaction.execute(
                "INSERT INTO note_tags(note_pk, tag)
                 SELECT notes.pk, ?1
                 FROM notes
                 JOIN json_each(?2) changed ON changed.value = notes.id",
                params![tag.as_str(), encoded_changed],
            )?,
            AddOrRemove::Remove(tag) => transaction.execute(
                "DELETE FROM note_tags
                 WHERE tag = ?1 AND note_pk IN (
                     SELECT notes.pk
                     FROM notes
                     JOIN json_each(?2) changed ON changed.value = notes.id
                 )",
                params![tag.as_str(), encoded_changed],
            )?,
        };
        let revision = next_revision(&transaction)?;
        let updated = timestamp_now()?;
        transaction.execute(
            "UPDATE notes SET updated = ?1, note_revision = ?2
             WHERE id IN (SELECT value FROM json_each(?3))",
            params![updated.as_str(), revision, encoded_changed],
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

    pub fn change_link(
        &mut self,
        id: &NoteId,
        operation: AddOrRemove<NoteId>,
        if_revision: Option<u64>,
    ) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source_pk = note_pk(&transaction, id)?;
        ensure_note_revision(&transaction, id, if_revision)?;
        let target = match &operation {
            AddOrRemove::Add(target) | AddOrRemove::Remove(target) => target,
        };
        if target == id {
            return Err(NtError::SelfLink);
        }
        let changed = match operation {
            AddOrRemove::Add(target) => {
                let target_pk = note_pk(&transaction, &target)?;
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
        touch_if_changed(&transaction, id, changed)?;
        transaction.commit()?;
        Ok(changed)
    }
}

fn touch_if_changed(transaction: &Transaction<'_>, id: &NoteId, changed: bool) -> Result<()> {
    if changed {
        let updated = timestamp_now()?;
        let revision = next_revision(transaction)?;
        transaction.execute(
            "UPDATE notes SET updated = ?1, note_revision = ?2 WHERE id = ?3",
            params![updated.as_str(), revision, id.to_string()],
        )?;
        record_change(transaction, revision, id, ChangeOperation::Metadata)?;
    }
    Ok(())
}
