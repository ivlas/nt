use std::collections::BTreeSet;
use std::fs;

use crate::error::{NtError, Result};
use crate::fs::{IndexMutationLock, atomic_write};
use crate::index::Index;
use crate::note::validate_id;

use super::note_ref;

pub(super) fn rm(ids: &[String]) -> Result<()> {
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let mut seen = BTreeSet::new();
    let mut notes = Vec::with_capacity(ids.len());

    for id in ids {
        validate_id(id)?;
        if !seen.insert(id.as_str()) {
            return Err(NtError::Message(format!("duplicate note id: {id}")));
        }

        let note = note_ref(&index, id)?.clone();
        let body = fs::read(&note.path)?;
        notes.push((note, body));
    }

    for (position, (note, _)) in notes.iter().enumerate() {
        if let Err(err) = fs::remove_file(&note.path) {
            let err = err.into();
            if let Err(rollback_err) = restore_removed_notes(&notes[..position]) {
                return Err(NtError::rollback_failed(
                    "removing note files",
                    err,
                    rollback_err,
                ));
            }
            return Err(err);
        }
    }

    index.remove_notes(ids.iter().map(String::as_str));
    if let Err(err) = index.save() {
        if err.is_write_committed_but_not_durable() {
            return Err(err);
        }
        if let Err(rollback_err) = restore_removed_notes(&notes) {
            return Err(NtError::rollback_failed("saving index", err, rollback_err));
        }
        return Err(err);
    }

    for id in ids {
        println!("removed {id}");
    }
    Ok(())
}

fn restore_removed_notes(notes: &[(crate::index::NoteMeta, Vec<u8>)]) -> Result<()> {
    for (note, body) in notes {
        atomic_write(&note.path, body)?;
    }
    Ok(())
}
