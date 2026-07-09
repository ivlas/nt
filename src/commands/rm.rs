use std::collections::BTreeSet;
use std::fs;

use crate::error::{NtError, Result};
use crate::index::Index;
use crate::note::validate_id;

use super::note_ref;

pub(super) fn rm(ids: &[String]) -> Result<()> {
    let mut index = Index::load()?;
    let mut seen = BTreeSet::new();
    let mut notes = Vec::with_capacity(ids.len());

    for id in ids {
        validate_id(id)?;
        if !seen.insert(id.as_str()) {
            return Err(NtError::Message(format!("duplicate note id: {id}")));
        }

        notes.push(note_ref(&index, id)?.clone());
    }

    for note in &notes {
        fs::remove_file(&note.path)?;
    }

    index.remove_notes(ids.iter().map(String::as_str));
    index.save()?;

    for id in ids {
        println!("removed {id}");
    }
    Ok(())
}
