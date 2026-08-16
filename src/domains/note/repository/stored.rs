use super::super::{CollectionPath, NoteId, Tag, Timestamp};
use crate::error::{NtError, Result};

pub(super) fn decode_collection(value: &str) -> Result<CollectionPath> {
    value.parse().map_err(|_| NtError::InvalidStoredNote)
}

pub(super) fn decode_id(value: &str) -> Result<NoteId> {
    value.parse().map_err(|_| NtError::InvalidStoredNote)
}

pub(super) fn decode_tag(value: &str) -> Result<Tag> {
    value.parse().map_err(|_| NtError::InvalidStoredNote)
}

pub(super) fn decode_timestamp(value: &str) -> Result<Timestamp> {
    value.parse().map_err(|_| NtError::InvalidStoredNote)
}
