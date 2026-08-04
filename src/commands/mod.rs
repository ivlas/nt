use std::fs;
use std::path::PathBuf;

use crate::cli::{Cli, Command};
use crate::error::{NtError, Result};
use crate::fs::nt_home;
use crate::note::{NoteId, Status};
use crate::repository::{NoteMeta, Repository};

mod add;
mod agenda;
mod export_cmd;
mod init;
mod list;
mod rm;
mod show;
mod update;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[]),
        Some(Command::Init { vault }) => init::init(&vault),
        Some(Command::Note { metadata }) => add::note(&metadata),
        Some(Command::Todo { metadata }) => add::todo(&metadata),
        Some(Command::List { args }) => list::list(&args),
        Some(Command::Find { expr }) => show::find(&expr),
        Some(Command::Show { id }) => show::show(&id),
        Some(Command::Rm { ids }) => rm::rm(&ids),
        Some(Command::Update { id, field, value }) => update::update(&id, field, value.as_deref()),
        Some(Command::Agenda { view }) => agenda::agenda(view),
        Some(Command::Export { path, ids }) => export_cmd::export(&path, &ids),
        Some(Command::Help { topic }) => crate::cli::help::print(&topic),
    }
}

fn ensure_note_exists(repository: &Repository, id: &NoteId) -> Result<()> {
    if repository.note_exists(id)? {
        Ok(())
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

fn push_unique_sorted<T: Eq + Ord>(values: &mut Vec<T>, value: T) {
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

fn apply_status_transition(note: &mut NoteMeta, status: Option<Status>, now: &str) {
    let is_terminal = status.is_some_and(Status::is_terminal);
    if is_terminal && note.status != status {
        note.closed = Some(now.to_string());
    } else if !is_terminal {
        note.closed = None;
    }
    note.status = status;
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
    crate::repository::parse_collection_name(collection).map(|_| ())
}

fn validate_tag(tag: &str) -> Result<()> {
    validate_lowercase_name(tag, "tag")
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
    use crate::repository::NoteMeta;

    pub fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.parse().unwrap(),
            "personal/inbox".to_string(),
            "# Storage shape\n".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage shape".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_status_transition, test_helpers::note};

    #[test]
    fn status_transitions_manage_closed_deterministically() {
        let mut note = note("018fbe0a-6c00-7000-8000-000000000001");
        apply_status_transition(
            &mut note,
            Some(crate::note::Status::Done),
            "2026-05-28T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));

        apply_status_transition(
            &mut note,
            Some(crate::note::Status::Done),
            "2026-05-29T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-28T15:00:00Z"));
        apply_status_transition(
            &mut note,
            Some(crate::note::Status::Dropped),
            "2026-05-30T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(
            &mut note,
            Some(crate::note::Status::Dropped),
            "2026-05-31T15:00:00Z",
        );
        assert_eq!(note.closed.as_deref(), Some("2026-05-30T15:00:00Z"));

        apply_status_transition(
            &mut note,
            Some(crate::note::Status::Open),
            "2026-06-01T15:00:00Z",
        );
        assert_eq!(note.closed, None);
    }
}
