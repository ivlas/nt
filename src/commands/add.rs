use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use crate::error::{NtError, Result};
use crate::fs::{IndexMutationLock, atomic_write, create_new_file};
use crate::index::{Index, NoteMeta};
use crate::note::{generate_unique_id, note_path, title_from_body, validate_id};

use super::{
    active_vault_path, add_body_sources, apply_status_transition, editor_temp_path,
    ensure_note_exists, push_unique_sorted, validate_collection, validate_priority,
    validate_status, validate_tag,
};

pub(super) fn note(metadata: &[String]) -> Result<()> {
    add(CreationKind::Note, metadata)
}

pub(super) fn todo(metadata: &[String]) -> Result<()> {
    add(CreationKind::Todo, metadata)
}

fn add(kind: CreationKind, metadata: &[String]) -> Result<()> {
    let body = read_note_body_for_create()?;
    let title = title_from_body(&body)?;
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let notes_dir = active_vault_path(&index)?.to_path_buf();
    let metadata = CreationMetadata::parse(kind, metadata, &index)?;
    let timestamp = generate_unique_id(&notes_dir, &index)?;
    let path = note_path(&notes_dir, &timestamp.id)?;
    let mut note = NoteMeta::new_note(
        timestamp.id.clone(),
        path.clone(),
        timestamp.iso.clone(),
        timestamp.iso.clone(),
        title,
    );
    metadata.apply(kind, &mut note, &timestamp.iso);
    add_body_sources(&mut note, &body);

    create_new_file(&path, body.as_bytes())?;

    index.upsert_note_with_body(note, &body);
    if let Err(err) = index.save() {
        let _ = fs::remove_file(&path);
        return Err(err);
    }

    println!("saved {}", timestamp.id);
    Ok(())
}

#[derive(Debug, Default)]
struct CreationMetadata {
    status: Option<String>,
    priority: Option<String>,
    scheduled: Option<String>,
    due: Option<String>,
    tags: Vec<String>,
    collections: Vec<String>,
    links: Vec<String>,
    sources: Vec<String>,
}

impl CreationMetadata {
    fn parse(kind: CreationKind, exprs: &[String], index: &Index) -> Result<Self> {
        let mut metadata = Self::default();

        for expr in exprs {
            metadata.parse_expr(kind, expr, index)?;
        }

        Ok(metadata)
    }

    fn parse_expr(&mut self, kind: CreationKind, expr: &str, index: &Index) -> Result<()> {
        let Some((field, value)) = expr.split_once(':') else {
            return Err(NtError::Message(format!(
                "unknown {kind} metadata `{expr}`; use tag:<tag>, collection:<name>, link:<id>, or source:<term>"
            )));
        };

        match field {
            "tag" => push_value_list(&mut self.tags, field, value),
            "collection" => {
                for collection in split_metadata_values(field, value)? {
                    validate_collection(&collection)?;
                    push_unique_sorted(&mut self.collections, collection);
                }
                Ok(())
            }
            "source" => push_single_value(&mut self.sources, field, value),
            "link" => {
                for link in split_metadata_values(field, value)? {
                    validate_id(&link)?;
                    ensure_note_exists(index, &link)?;
                    push_unique_sorted(&mut self.links, link);
                }
                Ok(())
            }
            "status" => {
                kind.ensure_todo_field(field)?;
                set_single_metadata(&mut self.status, field, value)?;
                validate_status(self.status.as_deref().unwrap_or_default())
            }
            "priority" => {
                kind.ensure_todo_field(field)?;
                set_single_metadata(&mut self.priority, field, value)?;
                validate_priority(self.priority.as_deref().unwrap_or_default())
            }
            "scheduled" => {
                kind.ensure_todo_field(field)?;
                set_single_metadata(&mut self.scheduled, field, value)?;
                crate::note::validate_date(self.scheduled.as_deref().unwrap_or_default())
            }
            "due" => {
                kind.ensure_todo_field(field)?;
                set_single_metadata(&mut self.due, field, value)?;
                crate::note::validate_date(self.due.as_deref().unwrap_or_default())
            }
            _ => Err(NtError::Message(format!(
                "unknown {kind} metadata field `{field}`"
            ))),
        }
    }

    fn apply(self, kind: CreationKind, note: &mut NoteMeta, now: &str) {
        if kind == CreationKind::Todo {
            note.kind = "todo".to_string();
        }
        apply_status_transition(note, self.status, now);
        note.priority = self.priority;
        note.scheduled = self.scheduled;
        note.due = self.due;
        note.tags = self.tags;
        note.collections = self.collections;
        note.links = self.links;
        note.sources = self.sources;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreationKind {
    Note,
    Todo,
}

impl CreationKind {
    fn ensure_todo_field(self, field: &str) -> Result<()> {
        if self == Self::Todo {
            Ok(())
        } else {
            Err(NtError::Message(format!(
                "`{field}` metadata is only valid for `nt todo`"
            )))
        }
    }
}

impl std::fmt::Display for CreationKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Note => formatter.write_str("note"),
            Self::Todo => formatter.write_str("todo"),
        }
    }
}

fn push_value_list(values: &mut Vec<String>, field: &str, raw: &str) -> Result<()> {
    for value in split_metadata_values(field, raw)? {
        if field == "tag" {
            validate_tag(&value)?;
        }
        push_unique_sorted(values, value);
    }
    Ok(())
}

fn push_single_value(values: &mut Vec<String>, field: &str, raw: &str) -> Result<()> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(NtError::Message(format!(
            "empty add metadata value for `{field}`"
        )));
    }

    push_unique_sorted(values, value.to_string());
    Ok(())
}

fn set_single_metadata(target: &mut Option<String>, field: &str, raw: &str) -> Result<()> {
    let values = split_metadata_values(field, raw)?;
    if values.len() != 1 {
        return Err(NtError::Message(format!(
            "`{field}` metadata accepts one value"
        )));
    }
    if target.replace(values[0].clone()).is_some() {
        return Err(NtError::Message(format!(
            "`{field}` metadata can be set only once"
        )));
    }
    Ok(())
}

fn split_metadata_values(field: &str, raw: &str) -> Result<Vec<String>> {
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();

    if values.is_empty() {
        return Err(NtError::Message(format!(
            "empty add metadata value for `{field}`"
        )));
    }

    Ok(values)
}

fn read_note_body_for_create() -> Result<String> {
    let mut body = String::new();

    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut body)?;
    } else {
        body = read_from_editor()?;
    }

    if body.trim().is_empty() {
        return Err(NtError::EmptyNote);
    }

    if !body.ends_with('\n') {
        body.push('\n');
    }

    Ok(body)
}

fn read_from_editor() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let path = add_temp_path()?;
    atomic_write(&path, b"")?;

    let status = ProcessCommand::new(&editor).arg(&path).status()?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        return Err(NtError::EditorFailed(editor));
    }

    let body = fs::read_to_string(&path)?;
    fs::remove_file(&path)?;
    Ok(body)
}

fn add_temp_path() -> Result<PathBuf> {
    editor_temp_path("note", None)
}

#[cfg(test)]
mod tests {
    use crate::index::Index;

    use super::{CreationKind, CreationMetadata};
    use crate::commands::test_helpers::note;

    #[test]
    fn creation_metadata_accepts_repeated_and_comma_separated_values() {
        let metadata = CreationMetadata::parse(
            CreationKind::Note,
            &[
                "tag:design,cli".to_string(),
                "tag:rust".to_string(),
                "collection:projects/nt".to_string(),
                "source:https://example.com/a,b".to_string(),
            ],
            &Index::default(),
        )
        .unwrap();
        let mut note = note("NT20260528T143012");

        metadata.apply(CreationKind::Note, &mut note, "2026-05-28T14:30:12Z");

        assert_eq!(note.tags, vec!["cli", "design", "rust"]);
        assert_eq!(note.collections, vec!["projects/nt"]);
        assert_eq!(note.sources, vec!["https://example.com/a,b"]);
        assert_eq!(note.kind, "note");
        assert_eq!(note.status, None);
    }

    #[test]
    fn creation_metadata_rejects_unknown_fields() {
        let err = CreationMetadata::parse(
            CreationKind::Note,
            &["topic:storage".to_string()],
            &Index::default(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "unknown note metadata field `topic`");

        let err = CreationMetadata::parse(
            CreationKind::Note,
            &["unknown".to_string()],
            &Index::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown note metadata"));

        let err =
            CreationMetadata::parse(CreationKind::Note, &["tag:".to_string()], &Index::default())
                .unwrap_err();
        assert_eq!(err.to_string(), "empty add metadata value for `tag`");

        let err = CreationMetadata::parse(
            CreationKind::Note,
            &["due:2026-06-30".to_string()],
            &Index::default(),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "`due` metadata is only valid for `nt todo`"
        );

        let err = CreationMetadata::parse(
            CreationKind::Note,
            &["link:NT99999999T999999".to_string()],
            &Index::default(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "note not found: NT99999999T999999");
    }

    #[test]
    fn todo_metadata_sets_kind_and_accepts_action_fields() {
        let metadata = CreationMetadata::parse(
            CreationKind::Todo,
            &[
                "status:open".to_string(),
                "priority:A".to_string(),
                "scheduled:2026-06-25".to_string(),
                "due:2026-06-30".to_string(),
            ],
            &Index::default(),
        )
        .unwrap();
        let mut note = note("NT20260528T143012");

        metadata.apply(CreationKind::Todo, &mut note, "2026-05-28T14:30:12Z");

        assert_eq!(note.kind, "todo");
        assert_eq!(note.status.as_deref(), Some("open"));
        assert_eq!(note.priority.as_deref(), Some("A"));
        assert_eq!(note.scheduled.as_deref(), Some("2026-06-25"));
        assert_eq!(note.due.as_deref(), Some("2026-06-30"));
    }
}
