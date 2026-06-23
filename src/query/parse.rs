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
    validate_id(&value.to_ascii_uppercase()).map_err(|_| {
        NtError::Message(format!(
            "invalid `{field}` note id `{value}`; use NTYYYYMMDDTHHmmss"
        ))
    })
}

pub(super) fn validate_id_prefix(value: &str) -> Result<()> {
    if value.len() > 17 || !value.starts_with("nt") {
        return Err(invalid_id_prefix(value));
    }

    for (index, byte) in value.bytes().enumerate() {
        let valid = match index {
            0 => byte == b'n',
            1 | 10 => byte == b't',
            2..=9 | 11..=16 => byte.is_ascii_digit(),
            _ => false,
        };
        if !valid {
            return Err(invalid_id_prefix(value));
        }
    }

    Ok(())
}

fn invalid_id_prefix(value: &str) -> NtError {
    NtError::Message(format!(
        "invalid `id` prefix `{value}`; use a prefix of NTYYYYMMDDTHHmmss"
    ))
}
