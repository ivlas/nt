use std::collections::BTreeSet;

use crate::error::{NtError, Result};
use crate::note::NoteId;
use crate::repository::Repository;

pub(super) fn rm(ids: &[String]) -> Result<()> {
    let mut parsed = Vec::with_capacity(ids.len());
    let mut unique = BTreeSet::new();
    for value in ids {
        let id: NoteId = value.parse()?;
        if !unique.insert(id.clone()) {
            return Err(NtError::DuplicateNoteId(value.clone()));
        }
        parsed.push(id);
    }
    let mut repository = Repository::open()?;
    repository.delete_notes(&parsed)?;
    println!("removed {}", parsed.len());
    Ok(())
}
