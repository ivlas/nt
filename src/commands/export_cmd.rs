use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::export::export_markdown;
use crate::fs::{absolute_path, atomic_write, relative_to_cwd};
use crate::index::Index;
use crate::note::validate_id;

use super::note_ref;

pub(super) fn export(path: &Path, ids: &[String]) -> Result<()> {
    let index = Index::load()?;
    let export_dir = absolute_path(path)?;
    fs::create_dir_all(&export_dir)?;
    let export_dir = fs::canonicalize(&export_dir)?;

    for id in export_ids(&index, ids)? {
        let note = note_ref(&index, &id)?;
        let path = export_dir.join(format!("{id}.md"));
        atomic_write(&path, export_markdown(note, &note.body)?.as_bytes())?;
        println!("exported {id} {}", relative_to_cwd(&path).display());
    }

    Ok(())
}

fn export_ids(index: &Index, ids: &[String]) -> Result<Vec<String>> {
    if ids.is_empty() {
        return Ok(index
            .all_notes()
            .iter()
            .map(|note| note.id.clone())
            .collect());
    }

    let mut seen = BTreeSet::new();
    let mut export_ids = Vec::new();
    for id in ids {
        validate_id(id)?;
        note_ref(index, id)?;
        if seen.insert(id.clone()) {
            export_ids.push(id.clone());
        }
    }

    Ok(export_ids)
}
