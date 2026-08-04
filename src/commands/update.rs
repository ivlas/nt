use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process::Command as ProcessCommand;

use crate::cli::UpdateField;
use crate::error::{NtError, Result};
use crate::fs::atomic_write;
use crate::repository::{NoteChange, Repository};

use super::{
    editor_temp_path, ensure_note_exists, validate_collection, validate_kind, validate_priority,
    validate_status, validate_tag,
};

#[derive(Debug)]
enum UpdateOperation {
    Kind(Option<String>),
    Status(Option<String>),
    Priority(Option<String>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Home(String),
    Set {
        field: UpdateField,
        add: bool,
        value: String,
    },
}

impl UpdateOperation {
    fn parse(field: UpdateField, raw: &str, repository: &Repository) -> Result<Self> {
        match field {
            UpdateField::Body => unreachable!(),
            UpdateField::Kind => {
                if raw != "-" {
                    validate_kind(raw)?;
                }
                Ok(Self::Kind((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Status => {
                if raw != "-" {
                    validate_status(raw)?;
                }
                Ok(Self::Status((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Priority => {
                if raw != "-" {
                    validate_priority(raw)?;
                }
                Ok(Self::Priority((raw != "-").then(|| raw.to_string())))
            }
            UpdateField::Scheduled | UpdateField::Due => {
                if raw != "-" {
                    crate::note::validate_date(raw)?;
                }
                let value = (raw != "-").then(|| raw.to_string());
                Ok(if matches!(field, UpdateField::Scheduled) {
                    Self::Scheduled(value)
                } else {
                    Self::Due(value)
                })
            }
            UpdateField::Home => {
                validate_collection(raw)?;
                Ok(Self::Home(raw.to_string()))
            }
            UpdateField::Tag
            | UpdateField::Collection
            | UpdateField::Link
            | UpdateField::Source => {
                let (add, value) = raw
                    .strip_prefix('+')
                    .map(|value| (true, value))
                    .or_else(|| raw.strip_prefix('-').map(|value| (false, value)))
                    .ok_or_else(|| {
                        NtError::Message(format!(
                            "`{}` update requires +value or -value",
                            field_name(field)
                        ))
                    })?;
                if value.is_empty() {
                    return Err(NtError::Message(format!(
                        "empty `{}` update value",
                        field_name(field)
                    )));
                }
                match field {
                    UpdateField::Tag => validate_tag(value)?,
                    UpdateField::Collection => validate_collection(value)?,
                    UpdateField::Link => {
                        crate::note::validate_id(value)?;
                        ensure_note_exists(repository, value)?;
                    }
                    UpdateField::Source => {}
                    _ => unreachable!(),
                }
                Ok(Self::Set {
                    field,
                    add,
                    value: value.to_string(),
                })
            }
        }
    }

    fn into_change(self) -> NoteChange {
        match self {
            Self::Kind(value) => NoteChange::Kind(value.unwrap_or_else(|| "note".to_string())),
            Self::Status(value) => NoteChange::Status(value),
            Self::Priority(value) => NoteChange::Priority(value),
            Self::Scheduled(value) => NoteChange::Scheduled(value),
            Self::Due(value) => NoteChange::Due(value),
            Self::Home(value) => NoteChange::Home(value),
            Self::Set { field, add, value } => match field {
                UpdateField::Tag => NoteChange::Tag { add, value },
                UpdateField::Collection => NoteChange::Collection { add, value },
                UpdateField::Link => NoteChange::Link { add, value },
                UpdateField::Source => NoteChange::Source { add, value },
                _ => unreachable!(),
            },
        }
    }
}

fn field_name(field: UpdateField) -> &'static str {
    match field {
        UpdateField::Body => "body",
        UpdateField::Kind => "kind",
        UpdateField::Status => "status",
        UpdateField::Priority => "priority",
        UpdateField::Scheduled => "scheduled",
        UpdateField::Due => "due",
        UpdateField::Tag => "tag",
        UpdateField::Collection => "collection",
        UpdateField::Home => "home",
        UpdateField::Link => "link",
        UpdateField::Source => "source",
    }
}

pub(super) fn update(id: &str, field: UpdateField, value: Option<&str>) -> Result<()> {
    crate::note::validate_id(id)?;
    let mut repository = Repository::open()?;

    if matches!(field, UpdateField::Body) {
        if value.is_some() {
            return Err(NtError::Message(
                "`body` update reads CommonMark from stdin or $EDITOR".to_string(),
            ));
        }
        let note = repository.get_note(id)?;
        let body = read_body(&note.body, id)?;
        let title = crate::note::title_from_body(&body)?;
        let now = crate::note::timestamp_now().iso;
        repository.update_note_body(id, &note.updated, &note.body, &body, &title, &now)?;
        println!("updated {id} body");
        return Ok(());
    }

    super::ensure_note_exists(&repository, id)?;
    let value = value.ok_or_else(|| {
        NtError::Message(format!("`{}` update requires a value", field_name(field)))
    })?;
    let operation = UpdateOperation::parse(field, value, &repository)?;
    let now = crate::note::timestamp_now().iso;
    repository.update_note(id, &operation.into_change(), &now)?;
    println!("updated {id} {} {value}", field_name(field));
    Ok(())
}

fn read_body(current: &str, id: &str) -> Result<String> {
    let mut body = String::new();
    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut body)?;
    } else {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let path = editor_temp_path("update-body", Some(id))?;
        atomic_write(&path, current.as_bytes())?;
        let status = ProcessCommand::new(&editor).arg(&path).status()?;
        if !status.success() {
            let _ = fs::remove_file(&path);
            return Err(NtError::EditorFailed(editor));
        }
        body = fs::read_to_string(&path)?;
        fs::remove_file(&path)?;
    }
    if body.trim().is_empty() {
        return Err(NtError::EmptyNote);
    }
    if !body.ends_with('\n') {
        body.push('\n');
    }
    Ok(body)
}
