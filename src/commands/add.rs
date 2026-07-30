use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use crate::error::{NtError, Result};
use crate::fs::atomic_write;
use crate::index::{Index, NoteMeta};
use crate::note::{new_id, title_from_body, validate_id};

use super::{
    add_body_sources, apply_status_transition, editor_temp_path, ensure_note_exists,
    push_unique_sorted, validate_collection, validate_priority, validate_status, validate_tag,
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
    let mut index = Index::load()?;
    let metadata = CreationMetadata::parse(kind, metadata, &index)?;
    let timestamp = crate::note::timestamp_now().iso;
    let home = metadata
        .home
        .clone()
        .or_else(|| metadata.collections.first().cloned())
        .map(Ok)
        .unwrap_or_else(|| index.default_home_collection())?;

    index.ensure_collection(&home, &timestamp)?;
    for collection in &metadata.collections {
        index.ensure_collection(collection, &timestamp)?;
    }

    let id = new_id();
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
    index.upsert_note(note);
    index.save()?;

    println!("saved {id}");
    Ok(())
}

#[derive(Debug, Default)]
struct CreationMetadata {
    home: Option<String>,
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
                "unknown {kind} metadata `{expr}`"
            )));
        };
        match field {
            "home" => {
                validate_collection(value)?;
                set_single_metadata(&mut self.home, field, value)
            }
            "tag" => push_value_list(&mut self.tags, field, value),
            "collection" => {
                for collection in split_metadata_values(field, value)? {
                    validate_collection(&collection)?;
                    if !self.collections.contains(&collection) {
                        self.collections.push(collection);
                    }
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
        let status = if kind == CreationKind::Todo && self.status.is_none() {
            Some("open".to_string())
        } else {
            self.status
        };
        if kind == CreationKind::Todo {
            note.kind = "todo".to_string();
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
