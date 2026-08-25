use rusqlite::Row;
use rusqlite::types::FromSql;

use super::super::{CollectionPath, NoteId, Tag, Timestamp};
use crate::error::{NtError, Result, StoredNoteContext};

pub(super) fn stored_value<T: FromSql>(
    row: &Row<'_>,
    index: usize,
    context: &StoredNoteContext,
    field: &'static str,
) -> Result<T> {
    row.get(index).map_err(|error| match error {
        error @ (rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::IntegralValueOutOfRange(..)
        | rusqlite::Error::Utf8Error(..)
        | rusqlite::Error::InvalidColumnType(..)) => {
            NtError::invalid_stored_with_source(context.clone(), field, error)
        }
        error => NtError::from(error),
    })
}

pub(super) fn decode_collection(
    value: &str,
    context: &StoredNoteContext,
) -> Result<CollectionPath> {
    value
        .parse()
        .map_err(|_| NtError::invalid_stored(context.clone(), "collection"))
}

pub(super) fn decode_id(value: &str, context: &StoredNoteContext) -> Result<NoteId> {
    value
        .parse()
        .map_err(|_| NtError::invalid_stored(context.clone(), "id"))
}

pub(super) fn decode_tag(value: &str, context: &StoredNoteContext) -> Result<Tag> {
    value
        .parse()
        .map_err(|_| NtError::invalid_stored(context.clone(), "tag"))
}

pub(super) fn decode_timestamp(
    value: &str,
    context: &StoredNoteContext,
    field: &'static str,
) -> Result<Timestamp> {
    value
        .parse()
        .map_err(|_| NtError::invalid_stored(context.clone(), field))
}

pub(super) fn decode_body_version(value: i64, context: &StoredNoteContext) -> Result<u64> {
    u64::try_from(value).map_err(|_| NtError::invalid_stored(context.clone(), "body_version"))
}

pub(super) fn decode_revision(value: i64, context: &StoredNoteContext) -> Result<u64> {
    let revision = u64::try_from(value)
        .map_err(|_| NtError::invalid_stored(context.clone(), "note_revision"))?;
    if revision == 0 {
        return Err(NtError::invalid_stored(context.clone(), "note_revision"));
    }
    Ok(revision)
}
