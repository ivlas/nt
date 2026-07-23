use std::collections::BTreeSet;
use std::fs;

use crate::error::{NtError, Result};
use crate::index::NoteMeta;

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

pub(super) fn matches_body(note: &NoteMeta, needle: &str) -> Result<bool> {
    let body = read_body(note)?.to_ascii_lowercase();
    let terms = tokenize_text(needle);
    if terms.is_empty() {
        return Ok(body.contains(needle));
    }

    Ok(terms.iter().all(|term| body.contains(term)))
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

fn tokenize_text(text: &str) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    let mut term = String::new();

    for char in text.chars() {
        if char.is_alphanumeric() {
            term.extend(char.to_lowercase());
        } else if !term.is_empty() {
            terms.insert(std::mem::take(&mut term));
        }
    }

    if !term.is_empty() {
        terms.insert(term);
    }

    terms
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::tokenize_text;

    #[test]
    fn tokenizes_text_to_lowercase_unique_terms() {
        assert_eq!(
            tokenize_text("QEMU, qemu; Firecracker/v1"),
            BTreeSet::from([
                "firecracker".to_string(),
                "qemu".to_string(),
                "v1".to_string()
            ])
        );
    }
}
