use rusqlite::{Transaction, TransactionBehavior, params};

use super::super::{NoteId, Tag, timestamp_now};
use crate::error::{NtError, Result};

use super::changes::{ChangeOperation, record_change};
use super::store::{next_revision, note_pk};
use super::{AddOrRemove, Repository};

impl Repository {
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
