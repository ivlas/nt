use std::collections::BTreeSet;

use crate::error::{NtError, Result};
use crate::note::NoteId;

use super::App;

pub(super) const STDIN_IDS: &str = "id:-";

pub(super) fn parse(app: &mut App<'_>) -> Result<Vec<NoteId>> {
    let input = app.input.read_stdin()?;
    let mut ids = Vec::new();
    let mut unique = BTreeSet::new();
    for line in input.lines() {
        let id: NoteId = line.parse()?;
        if !unique.insert(id.clone()) {
            return Err(NtError::DuplicateNoteId(line.to_string()));
        }
        ids.push(id);
    }
    if ids.is_empty() {
        return Err(NtError::InvalidValue {
            field: "stdin IDs",
            value: "empty".to_string(),
        });
    }
    Ok(ids)
}

pub(super) fn reject_precondition(if_revision: Option<&str>) -> Result<()> {
    if let Some(value) = if_revision {
        return Err(NtError::InvalidValue {
            field: "revision precondition",
            value: format!("{value} with id:-"),
        });
    }
    Ok(())
}
