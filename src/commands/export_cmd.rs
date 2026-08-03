use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::export::export_markdown;
use crate::fs::{absolute_path, atomic_write, relative_to_cwd};
use crate::note::validate_id;
use crate::repository::Repository;

pub(super) fn export(path: &Path, ids: &[String]) -> Result<()> {
    let repository = Repository::open()?;
    let export_dir = absolute_path(path)?;
    fs::create_dir_all(&export_dir)?;
    let export_dir = fs::canonicalize(&export_dir)?;

    let notes = if ids.is_empty() {
        repository.list_notes()?
    } else {
        let export_ids = export_ids(&repository, ids)?;
        export_ids
            .iter()
            .map(|id| repository.get_note(id))
            .collect::<Result<Vec<_>>>()?
    };
    for note in notes {
        let id = &note.id;
        let path = export_dir.join(format!("{id}.md"));
        atomic_write(&path, export_markdown(&note, &note.body)?.as_bytes())?;
        println!("exported {id} {}", relative_to_cwd(&path).display());
    }

    Ok(())
}

fn export_ids(repository: &Repository, ids: &[String]) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut export_ids = Vec::new();
    for id in ids {
        validate_id(id)?;
        if !repository.note_exists(id)? {
            return Err(crate::error::NtError::NoteNotFound(id.clone()));
        }
        if seen.insert(id.clone()) {
            export_ids.push(id.clone());
        }
    }

    Ok(export_ids)
}
