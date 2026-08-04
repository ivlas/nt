use crate::error::{NtError, Result};
use crate::note::NoteId;
use crate::repository::Repository;
use std::collections::BTreeSet;

pub(super) fn rm(ids: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();

    for id in ids {
        let note_id: NoteId = id.parse()?;
        if !seen.insert(note_id.clone()) {
            return Err(NtError::Message(format!("duplicate note id: {id}")));
        }
        parsed.push(note_id);
    }

    let mut repository = Repository::open()?;
    repository.delete_notes(&parsed)?;

    for id in ids {
        println!("removed {id}");
    }
    Ok(())
}
