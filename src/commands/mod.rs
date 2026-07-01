use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::completion::print_completion;
use crate::cli::{Cli, Command};
use crate::error::{NtError, Result};
use crate::fs::nt_home;
use crate::index::{Index, NoteMeta};

mod add;
mod agenda;
mod config;
mod export_cmd;
mod init;
mod list;
mod rm;
mod show;
mod update;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[]),
        Some(Command::Init { notes_dir }) => init::init(&notes_dir),
        Some(Command::Note { metadata }) => add::note(&metadata),
        Some(Command::Todo { metadata }) => add::todo(&metadata),
        Some(Command::Rebuild) => init::rebuild(),
        Some(Command::List { args }) => list::list(&args),
        Some(Command::Find { expr }) => show::find(&expr),
        Some(Command::Show { id }) => show::show(&id),
        Some(Command::Open { id }) => show::open(&id),
        Some(Command::Rm { ids }) => rm::rm(&ids),
        Some(Command::Update { id, field, value }) => update::update(&id, field, &value),
        Some(Command::Agenda { view }) => agenda::agenda(view),
        Some(Command::Export { path, ids }) => export_cmd::export(&path, &ids),
        Some(Command::Config { command }) => config::config(command),
        Some(Command::Completion { shell }) => {
            print_completion(shell);
            Ok(())
        }
        Some(Command::Help { topic }) => crate::cli::help::print(&topic),
    }
}

fn active_vault_path(index: &Index) -> Result<&Path> {
    index.active_vault_path().ok_or(NtError::MissingVault)
}

fn note_mut<'a>(index: &'a mut Index, id: &str) -> Result<&'a mut NoteMeta> {
    let in_active_vault = {
        let note = index
            .notes
            .get(id)
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
        index.note_is_in_active_vault(note)
    };
    if !in_active_vault {
        return Err(NtError::NoteNotFound(id.to_string()));
    }

    index
        .notes
        .get_mut(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))
}

fn note_ref<'a>(index: &'a Index, id: &str) -> Result<&'a NoteMeta> {
    let note = index
        .notes
        .get(id)
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    if index.note_is_in_active_vault(note) {
        Ok(note)
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

fn ensure_note_exists(index: &Index, id: &str) -> Result<()> {
    note_ref(index, id).map(|_| ())
}

fn push_unique_sorted(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
        values.sort();
    }
}

fn add_body_sources(note: &mut NoteMeta, body: &str) {
    for source in crate::note::sources_from_body(body) {
        push_unique_sorted(&mut note.sources, source);
    }
}

fn apply_status_transition(note: &mut NoteMeta, status: Option<String>, now: &str) {
    let is_terminal = status.as_deref().is_some_and(is_terminal_status);
    if is_terminal && note.status != status {
        note.closed = Some(now.to_string());
    } else if !is_terminal {
        note.closed = None;
    }
    note.status = status;
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "done" | "dropped")
}

fn validate_lowercase_name(value: &str, kind: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(NtError::Message(format!("empty {kind} name")));
    }

    if value
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_uppercase() || ch == ',')
    {
        return Err(NtError::Message(format!(
            "invalid {kind} `{value}`; use lowercase names without spaces or commas"
        )));
    }

    Ok(())
}

fn validate_collection(collection: &str) -> Result<()> {
    validate_lowercase_name(collection, "collection")
}

fn validate_tag(tag: &str) -> Result<()> {
    validate_lowercase_name(tag, "tag")
}

fn validate_source(source: &str) -> Result<()> {
    if source.trim().is_empty() {
        return Err(NtError::Message("empty source value".to_string()));
    }
    Ok(())
}

fn validate_kind(kind: &str) -> Result<()> {
    if matches!(kind, "note" | "todo") {
        Ok(())
    } else {
        Err(NtError::Message(format!("invalid kind: {kind}")))
    }
}

fn validate_status(status: &str) -> Result<()> {
    if matches!(status, "open" | "waiting" | "done" | "dropped") {
        Ok(())
    } else {
        Err(NtError::Message(format!("invalid status: {status}")))
    }
}

fn validate_priority(priority: &str) -> Result<()> {
    if matches!(priority, "S" | "A" | "B" | "C" | "D") {
        Ok(())
    } else {
        Err(NtError::Message(format!(
            "invalid priority `{priority}`; use S, A, B, C, or D"
        )))
    }
}

fn editor_temp_path(action: &str, id: Option<&str>) -> Result<PathBuf> {
    let dir = nt_home()?;
    fs::create_dir_all(&dir)?;
    let file_name = match id {
        Some(id) => format!(".nt-{action}-{id}-{}.tmp", std::process::id()),
        None => format!(".nt-{action}-{}.tmp", std::process::id()),
    };
    Ok(dir.join(file_name))
}

#[cfg(test)]
mod test_helpers {
    use std::path::PathBuf;

    use crate::index::{Index, NoteMeta, VaultMeta};

    pub fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage shape".to_string(),
        )
    }

    pub fn active_index(notes: Vec<NoteMeta>) -> Index {
        let mut index = Index::default();
        index.active_vault = Some("notes".to_string());
        index.vaults.insert(
            "notes".to_string(),
            VaultMeta {
                path: PathBuf::from("notes"),
                created: "2026-05-01T00:00:00Z".to_string(),
            },
        );
        for note in notes {
            index.upsert_note(note);
        }
        index
    }

    pub fn todo(
        id: &str,
        status: &str,
        priority: Option<&str>,
        scheduled: Option<&str>,
        due: Option<&str>,
    ) -> NoteMeta {
        let mut note = note(id);
        note.created = crate::note::iso_from_id(id).unwrap();
        note.kind = "todo".to_string();
        note.status = Some(status.to_string());
        note.priority = priority.map(str::to_string);
        note.scheduled = scheduled.map(str::to_string);
        note.due = due.map(str::to_string);
        note
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_status_transition, test_helpers::note};

    #[test]
    fn status_transitions_manage_closed_deterministically() {
        let mut note = note("NT20260528T143012");
        apply_status_transition(&mut note, Some("done".to_string()), "2026-05-28T15:00:00Z");
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));

        apply_status_transition(&mut note, Some("done".to_string()), "2026-05-29T15:00:00Z");
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));
        apply_status_transition(
            &mut note,
            Some("dropped".to_string()),
            "2026-05-30T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(
            &mut note,
            Some("dropped".to_string()),
            "2026-05-31T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(&mut note, Some("open".to_string()), "2026-06-01T15:00:00Z");
        assert_eq!(note.closed, None);
    }
}
