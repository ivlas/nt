use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{NtError, Result};
use crate::fs::{IndexMutationLock, absolute_path, nt_home, relative_to_cwd};
use crate::index::{Index, NoteMeta};
use crate::note::{title_from_body, validate_id};

use super::{active_vault_path, add_body_sources};

pub(super) fn init(notes_dir: &Path) -> Result<()> {
    let notes_dir = absolute_path(notes_dir)?;
    ensure_notes_dir_is_flat(&notes_dir)?;

    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let timestamp = crate::note::timestamp_now();
    let vault = index.create_vault_for_path(notes_dir.clone(), timestamp.iso)?;

    fs::create_dir_all(&notes_dir)?;
    fs::create_dir_all(nt_home()?)?;

    index.active_vault = Some(vault.clone());
    import_existing_notes(&mut index, &notes_dir)?;
    index.save()?;

    println!(
        "initialized {vault} {}",
        relative_to_cwd(&notes_dir).display()
    );
    Ok(())
}

fn import_existing_notes(index: &mut Index, notes_dir: &Path) -> Result<()> {
    for path in valid_note_paths(notes_dir)? {
        let id = id_from_note_path(&path)?;
        if let Some(existing) = index.notes.get(&id)
            && existing.path != path
        {
            return Err(NtError::Message(format!(
                "note id `{id}` already exists in index at {}",
                existing.path.display()
            )));
        }

        let (note, body) = note_meta_from_markdown(index.notes.get(&id), &path)?;
        index.upsert_note_with_body(note, &body);
    }

    Ok(())
}

pub(super) fn rebuild() -> Result<()> {
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let notes_dir = active_vault_path(&index)?.to_path_buf();
    ensure_notes_dir_is_flat(&notes_dir)?;
    let mut rebuilt_notes = BTreeMap::new();
    let mut rebuilt_bodies = BTreeMap::new();

    for path in valid_note_paths(&notes_dir)? {
        let id = id_from_note_path(&path)?;
        let (note, body) = note_meta_from_markdown(index.notes.get(&id), &path)?;
        rebuilt_bodies.insert(id.clone(), body);
        rebuilt_notes.insert(id, note);
    }

    let count = rebuilt_notes.len();
    index.replace_active_vault_notes_with_bodies(rebuilt_notes, &rebuilt_bodies);
    index.save()?;

    println!("rebuilt {count}");
    Ok(())
}

fn valid_note_paths(notes_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(notes_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        let stem = path.file_stem().and_then(|value| value.to_str());
        let extension = path.extension().and_then(|value| value.to_str());
        if extension == Some("md") && stem.is_some_and(|value| validate_id(value).is_ok()) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn id_from_note_path(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| NtError::Message(format!("invalid note filename: {}", path.display())))
}

fn note_meta_from_markdown(existing: Option<&NoteMeta>, path: &Path) -> Result<(NoteMeta, String)> {
    let id = id_from_note_path(path)?;
    let created = crate::note::iso_from_id(&id)?;
    let updated = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(crate::note::timestamp_from_system_time)
        .map(|timestamp| timestamp.iso)
        .unwrap_or_else(|_| created.clone());
    let body = fs::read_to_string(path)?;

    let mut note = NoteMeta::new_note(
        id,
        path.to_path_buf(),
        created,
        updated,
        title_from_body(&body)?,
    );
    if let Some(existing) = existing {
        note.kind = existing.kind.clone();
        note.status = existing.status.clone();
        note.priority = existing.priority.clone();
        note.scheduled = existing.scheduled.clone();
        note.due = existing.due.clone();
        note.closed = existing.closed.clone();
        note.tags = existing.tags.clone();
        note.collections = existing.collections.clone();
        note.links = existing.links.clone();
        note.sources = existing.sources.clone();
    }
    add_body_sources(&mut note, &body);
    Ok((note, body))
}

fn ensure_notes_dir_is_flat(notes_dir: &Path) -> Result<()> {
    if !notes_dir.exists() {
        return Ok(());
    }

    if !notes_dir.is_dir() {
        return Err(NtError::Message(format!(
            "notes path is not a directory: {}",
            notes_dir.display()
        )));
    }

    for entry in fs::read_dir(notes_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let stem = path.file_stem().and_then(|value| value.to_str());
        let extension = path.extension().and_then(|value| value.to_str());

        if !file_type.is_file()
            || extension != Some("md")
            || stem.is_none_or(|value| validate_id(value).is_err())
        {
            return Err(NtError::Message(format!(
                "notes directory must contain only NTYYYYMMDDTHHmmss.md files; invalid entry: {}",
                path.display()
            )));
        }
    }

    Ok(())
}
