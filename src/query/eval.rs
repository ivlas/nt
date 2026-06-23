use std::fs;

use crate::error::{NtError, Result};
use crate::index::{Index, NoteMeta, tokenize_text};

use super::parse::normalize;

pub(super) fn contains_normalized(values: &[String], needle: &str) -> bool {
    values.iter().any(|value| normalize(value) == needle)
}

pub(super) fn matches_metadata(note: &NoteMeta, needle: &str) -> bool {
    note.id.to_ascii_lowercase().contains(needle)
        || note.title.to_ascii_lowercase().contains(needle)
        || note.kind.to_ascii_lowercase().contains(needle)
        || note
            .status
            .as_deref()
            .is_some_and(|status| status.to_ascii_lowercase().contains(needle))
        || note
            .priority
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains(needle))
        || note
            .scheduled
            .as_deref()
            .is_some_and(|value| value.contains(needle))
        || note
            .due
            .as_deref()
            .is_some_and(|value| value.contains(needle))
        || note
            .closed
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains(needle))
        || note
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(needle))
        || note
            .collections
            .iter()
            .any(|collection| collection.to_ascii_lowercase().contains(needle))
        || note
            .links
            .iter()
            .any(|link| link.to_ascii_lowercase().contains(needle))
        || note
            .sources
            .iter()
            .any(|reference| reference.to_ascii_lowercase().contains(needle))
}

pub(super) fn matches_body(index: &Index, note: &NoteMeta, needle: &str) -> Result<bool> {
    let terms: Vec<String> = tokenize_text(needle).into_iter().collect();
    if let Some(matches) = index.body_terms_match(&note.id, &terms) {
        if matches && !note.path.exists() {
            read_body(note)?;
        }
        return Ok(matches);
    }

    let body = read_body(note)?;

    Ok(body.to_ascii_lowercase().contains(needle))
}

fn read_body(note: &NoteMeta) -> Result<String> {
    fs::read_to_string(&note.path).map_err(|err| {
        NtError::Message(format!(
            "note body not readable for {} at {}: {err}",
            note.id,
            note.path.display()
        ))
    })
}
