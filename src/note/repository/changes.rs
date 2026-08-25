use std::fmt;

use rusqlite::{Transaction, params};

use super::Repository;
use super::stored::{decode_id, decode_revision, stored_value};
use crate::error::{NtError, Result, StoredNoteContext};
use crate::note::NoteId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeOperation {
    Add,
    Edit,
    Metadata,
    Remove,
}

impl ChangeOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Edit => "edit",
            Self::Metadata => "metadata",
            Self::Remove => "remove",
        }
    }

    fn decode(value: &str, context: &StoredNoteContext) -> Result<Self> {
        match value {
            "add" => Ok(Self::Add),
            "edit" => Ok(Self::Edit),
            "metadata" => Ok(Self::Metadata),
            "remove" => Ok(Self::Remove),
            _ => Err(NtError::invalid_stored(context.clone(), "change operation")),
        }
    }
}

impl fmt::Display for ChangeOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    revision: u64,
    operation: ChangeOperation,
    note_id: NoteId,
}

impl Change {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn operation(&self) -> ChangeOperation {
        self.operation
    }

    pub fn note_id(&self) -> &NoteId {
        &self.note_id
    }
}

impl Repository {
    pub fn visit_changes_since(
        &self,
        revision: u64,
        mut visit: impl FnMut(Change) -> Result<()>,
    ) -> Result<()> {
        let revision = i64::try_from(revision).map_err(|_| NtError::InvalidValue {
            field: "revision",
            value: revision.to_string(),
        })?;
        let mut statement = self.connection.prepare(
            "SELECT revision, note_id, operation
             FROM note_changes
             WHERE revision > ?1
             ORDER BY revision ASC, note_id ASC",
        )?;
        let mut rows = statement.query([revision])?;
        while let Some(row) = rows.next()? {
            let id_value = row.get::<_, String>(1)?;
            let context = StoredNoteContext::new(Some(id_value.clone()), None);
            let revision =
                decode_revision(stored_value(row, 0, &context, "change revision")?, &context)?;
            let note_id = decode_id(&id_value, &context)?;
            let operation = ChangeOperation::decode(
                &stored_value::<String>(row, 2, &context, "change operation")?,
                &context,
            )?;
            visit(Change {
                revision,
                operation,
                note_id,
            })?;
        }
        Ok(())
    }
}

pub(super) fn record_change(
    transaction: &Transaction<'_>,
    revision: i64,
    id: &NoteId,
    operation: ChangeOperation,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO note_changes(revision, note_id, operation) VALUES (?1, ?2, ?3)",
        params![revision, id.to_string(), operation.as_str()],
    )?;
    Ok(())
}
