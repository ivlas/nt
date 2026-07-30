use crate::error::{NtError, Result};
use crate::note::validate_id;

pub(super) fn unknown_field_error(field: &str) -> String {
    match super::suggest::query_field_suggestion(field) {
        Some(suggestion) => {
            format!("unknown query field `{field}`; did you mean `{suggestion}`?")
        }
        None => format!("unknown query field `{field}`"),
    }
}

pub(super) fn normalize(value: &str) -> String {
    value.to_ascii_lowercase()
}

pub(super) fn validate_date_value(field: &str, value: &str) -> Result<()> {
    crate::note::validate_date(value)
        .map_err(|_| NtError::Message(format!("invalid `{field}` date `{value}`; use YYYY-MM-DD")))
}

pub(super) fn validate_priority(value: &str) -> Result<()> {
    if matches!(
        value.to_ascii_uppercase().as_str(),
        "S" | "A" | "B" | "C" | "D"
    ) {
        Ok(())
    } else {
        Err(NtError::Message(format!(
            "invalid priority `{value}`; use S, A, B, C, or D"
        )))
    }
}

pub(super) fn validate_note_id_value(field: &str, value: &str) -> Result<()> {
    validate_id(value)
        .map_err(|_| NtError::Message(format!("invalid `{field}` note id `{value}`; use a UUIDv7")))
}

pub(super) fn validate_id_prefix(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 36 {
        return Err(invalid_id_prefix(value));
    }
    if !value.bytes().enumerate().all(|(index, byte)| {
        matches!(index, 8 | 13 | 18 | 23) && byte == b'-' || byte.is_ascii_hexdigit()
    }) {
        return Err(invalid_id_prefix(value));
    }

    Ok(())
}

fn invalid_id_prefix(value: &str) -> NtError {
    NtError::Message(format!(
        "invalid `id` prefix `{value}`; use a UUIDv7 prefix"
    ))
}
