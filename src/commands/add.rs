use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use crate::error::{NtError, Result};
use crate::fs::atomic_write;
use crate::note::{
    Date, NoteId, NoteKind, Priority, QualifiedCollection, Status, Timestamp, title_from_body,
};
use crate::repository::{NoteMeta, Repository};

use super::{
    add_body_sources, apply_status_transition, editor_temp_path, ensure_note_exists,
    push_unique_sorted, validate_tag,
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
    let mut repository = Repository::open()?;
    let metadata = CreationMetadata::parse(kind, metadata, &repository)?;
    let timestamp = crate::note::timestamp_now();
    let home = metadata
        .home
        .clone()
        .or_else(|| metadata.collections.first().cloned())
        .map(Ok)
        .unwrap_or_else(|| repository.default_home_collection())?;

    let id = NoteId::generate();
    let mut note = NoteMeta::new_note(
        id.clone(),
        home,
        body.clone(),
        timestamp.clone(),
        timestamp.clone(),
        title,
    );
    metadata.apply(kind, &mut note, &timestamp);
    add_body_sources(&mut note, &body);
    repository.insert_note(&note)?;

    println!("saved {id}");
    Ok(())
}

#[derive(Debug, Default)]
struct CreationMetadata {
    home: Option<QualifiedCollection>,
    status: Option<Status>,
    priority: Option<Priority>,
    scheduled: Option<Date>,
    due: Option<Date>,
    tags: Vec<String>,
    collections: Vec<QualifiedCollection>,
    links: Vec<NoteId>,
    sources: Vec<String>,
}

impl CreationMetadata {
    fn parse(kind: CreationKind, exprs: &[String], repository: &Repository) -> Result<Self> {
        let mut metadata = Self::default();
        for expr in exprs {
            metadata.parse_expr(kind, expr, repository)?;
        }
        Ok(metadata)
    }

    fn parse_expr(
        &mut self,
        kind: CreationKind,
        expr: &str,
        repository: &Repository,
    ) -> Result<()> {
        let Some((field, value)) = expr.split_once(':') else {
            return Err(NtError::Message(format!(
                "unknown {kind} metadata `{expr}`"
            )));
        };
        match field {
            "home" => set_typed_metadata(&mut self.home, field, value),
            "tag" => push_value_list(&mut self.tags, field, value),
            "collection" => {
                for collection in split_metadata_values(field, value)? {
                    let collection = collection.parse()?;
                    if !self.collections.contains(&collection) {
                        self.collections.push(collection);
                    }
                }
                Ok(())
            }
            "source" => push_single_value(&mut self.sources, field, value),
            "link" => {
                for link in split_metadata_values(field, value)? {
                    let link = link.parse()?;
                    ensure_note_exists(repository, &link)?;
                    push_unique_sorted(&mut self.links, link);
                }
                Ok(())
            }
            "status" => {
                kind.ensure_todo_field(field)?;
                set_typed_metadata(&mut self.status, field, value)
            }
            "priority" => {
                kind.ensure_todo_field(field)?;
                set_typed_metadata(&mut self.priority, field, value)
            }
            "scheduled" => {
                kind.ensure_todo_field(field)?;
                set_typed_metadata(&mut self.scheduled, field, value)
            }
            "due" => {
                kind.ensure_todo_field(field)?;
                set_typed_metadata(&mut self.due, field, value)
            }
            _ => Err(NtError::Message(format!(
                "unknown {kind} metadata field `{field}`"
            ))),
        }
    }

    fn apply(self, kind: CreationKind, note: &mut NoteMeta, now: &Timestamp) {
        let status = if kind == CreationKind::Todo && self.status.is_none() {
            Some(Status::Open)
        } else {
            self.status
        };
        if kind == CreationKind::Todo {
            note.kind = NoteKind::Todo;
        }
        apply_status_transition(note, status, now);
        note.priority = self.priority;
        note.scheduled = self.scheduled;
        note.due = self.due;
        note.tags = self.tags;
        for collection in self.collections {
            push_unique_sorted(&mut note.collections, collection);
        }
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
        formatter.write_str(match self {
            Self::Note => "note",
            Self::Todo => "todo",
        })
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

fn set_typed_metadata<T>(target: &mut Option<T>, field: &str, raw: &str) -> Result<()>
where
    T: std::str::FromStr<Err = NtError>,
{
    let values = split_metadata_values(field, raw)?;
    if values.len() != 1 {
        return Err(NtError::Message(format!(
            "`{field}` metadata accepts one value"
        )));
    }
    let value = values[0].parse()?;
    if target.replace(value).is_some() {
        return Err(NtError::Message(format!(
            "`{field}` metadata can be set only once"
        )));
    }
    Ok(())
}

fn split_metadata_values(field: &str, raw: &str) -> Result<Vec<String>> {
    let values: Vec<_> = raw
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
