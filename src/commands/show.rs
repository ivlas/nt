use std::collections::BTreeSet;
use std::fs;
use std::process::Command as ProcessCommand;

use crate::display::{joined_or_dash, summary_line};
use crate::error::{NtError, Result};
use crate::fs::{IndexMutationLock, atomic_write, relative_to_cwd};
use crate::index::Index;
use crate::note::{title_from_body, validate_id};
use crate::query::Query;
use crate::terminal::{Style, paint};

use super::{add_body_sources, editor_temp_path, note_ref};

pub(super) fn show(id: &str) -> Result<()> {
    let text = show_text_for_display(id, crate::terminal::stdout_color_enabled())?;

    print!("{text}");
    if !text.ends_with('\n') {
        println!();
    }

    Ok(())
}

fn show_text_for_display(id: &str, color: bool) -> Result<String> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = note_ref(&index, id)?;
    let body = fs::read_to_string(&note.path)?;

    let mut text = String::new();
    text.push_str(&format!(
        "{}  {}\n",
        paint(&note.id, Style::BrightCyan, color),
        note.title
    ));
    text.push_str(&format!(
        "path {}\n",
        paint(
            &relative_to_cwd(&note.path).display().to_string(),
            Style::Dim,
            color
        )
    ));
    text.push_str(&format!(
        "created {}\n",
        paint(&note.created, Style::Dim, color)
    ));
    text.push_str(&format!(
        "updated {}\n",
        paint(&note.updated, Style::Dim, color)
    ));
    text.push_str(&format!("kind {}\n", note.kind));
    text.push_str(&format!(
        "status {}\n",
        note.status.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!(
        "priority {}\n",
        note.priority.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!(
        "scheduled {}\n",
        note.scheduled.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!("due {}\n", note.due.as_deref().unwrap_or("-")));
    text.push_str(&format!(
        "closed {}\n",
        note.closed.as_deref().unwrap_or("-")
    ));
    text.push_str(&format!(
        "tags {}\n",
        paint(&joined_or_dash(&note.tags), Style::Green, color)
    ));
    text.push_str(&format!(
        "collections {}\n",
        joined_or_dash(&note.collections)
    ));
    text.push_str(&format!("links {}\n", joined_or_dash(&note.links)));
    text.push_str(&format!("sources {}\n\n", joined_or_dash(&note.sources)));
    text.push_str(&body);
    if !text.ends_with('\n') {
        text.push('\n');
    }

    Ok(text)
}

pub(super) fn open(id: &str) -> Result<()> {
    validate_id(id)?;
    let index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let body = fs::read_to_string(&note.path)?;
    let original_body = body.as_bytes().to_vec();
    let open_path = open_temp_path(id)?;
    atomic_write(&open_path, body.as_bytes())?;

    let status = ProcessCommand::new(&editor).arg(&open_path).status()?;
    if !status.success() {
        let _ = fs::remove_file(&open_path);
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&open_path)?;
    if body.trim().is_empty() {
        let _ = fs::remove_file(&open_path);
        return Err(NtError::EmptyNote);
    }
    let title = match title_from_body(&body) {
        Ok(title) => title,
        Err(err) => {
            let _ = fs::remove_file(&open_path);
            return Err(err);
        }
    };
    let _ = fs::remove_file(&open_path);

    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let note = note_ref(&index, id)?.clone();
    let current_body = fs::read(&note.path)?;
    if current_body != original_body {
        return Err(NtError::Message(
            "note changed during edit; please retry".to_string(),
        ));
    }

    let note_write_error = match atomic_write(&note.path, body.as_bytes()) {
        Ok(()) => None,
        Err(err) if err.is_write_committed_but_not_durable() => Some(err),
        Err(err) => return Err(err),
    };
    let timestamp = crate::note::timestamp_now();
    let note_path = note.path.clone();
    let mut updated = note;
    updated.updated = timestamp.iso;
    updated.title = title;
    add_body_sources(&mut updated, &body);

    index.upsert_note_with_body(updated, &body);
    if let Err(err) = index.save() {
        if err.is_write_committed_but_not_durable() {
            return Err(err);
        }
        if let Some(note_write_error) = note_write_error {
            return Err(NtError::partial_commit(
                "saving index after editing note",
                note_write_error,
                err,
            ));
        }
        if let Err(rollback_err) = atomic_write(&note_path, &original_body) {
            return Err(NtError::rollback_failed("saving index", err, rollback_err));
        }
        return Err(err);
    }

    if let Some(note_write_error) = note_write_error {
        return Err(note_write_error);
    }

    println!("saved {id}");
    Ok(())
}

pub(super) fn find(exprs: &[String]) -> Result<()> {
    let index = Index::load()?;
    let query = Query::parse(exprs)?;
    let candidates = query.candidate_ids(&index);

    if candidates.as_ref().is_some_and(BTreeSet::is_empty) {
        return Ok(());
    }

    for note in index.active_recent_notes() {
        if !candidates.as_ref().is_none_or(|ids| ids.contains(&note.id)) {
            continue;
        }

        if query.matches(&index, note)? {
            println!("{}", summary_line(note));
        }
    }

    Ok(())
}

fn open_temp_path(id: &str) -> Result<std::path::PathBuf> {
    editor_temp_path("open", Some(id))
}
