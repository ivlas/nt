use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::{NtError, Result};
use crate::export::export_markdown;
use crate::fs::{absolute_path, atomic_write, relative_to_cwd};
use crate::index::Index;
use crate::note::validate_id;

use super::{active_vault_path, note_ref};

pub(super) fn export(path: &Path, ids: &[String]) -> Result<()> {
    let index = Index::load()?;
    let active_vault = active_vault_path(&index)?.to_path_buf();
    let export_dir = absolute_path(path)?;

    ensure_export_dir_is_not_active_vault(&export_dir, &active_vault)?;
    fs::create_dir_all(&export_dir)?;
    let export_dir = fs::canonicalize(&export_dir)?;
    let active_vault = fs::canonicalize(&active_vault)?;
    ensure_export_dir_is_not_active_vault(&export_dir, &active_vault)?;

    for id in export_ids(&index, ids)? {
        let note = note_ref(&index, &id)?;
        let body = fs::read_to_string(&note.path)?;
        let path = export_dir.join(format!("{id}.md"));
        atomic_write(&path, export_markdown(note, &body)?.as_bytes())?;
        println!("exported {id} {}", relative_to_cwd(&path).display());
    }

    Ok(())
}

fn ensure_export_dir_is_not_active_vault(export_dir: &Path, active_vault: &Path) -> Result<()> {
    if export_dir == active_vault || export_dir.starts_with(active_vault) {
        return Err(NtError::Message(
            "export path must be outside the active notes directory".to_string(),
        ));
    }

    Ok(())
}

fn export_ids(index: &Index, ids: &[String]) -> Result<Vec<String>> {
    if ids.is_empty() {
        return Ok(index
            .recent
            .iter()
            .filter_map(|id| {
                let note = index.notes.get(id)?;
                index.note_is_in_active_vault(note).then(|| id.clone())
            })
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
