use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process::Command as ProcessCommand;

use crate::cli::{SetUpdateField, UpdateField, ValueUpdateField};
use crate::error::{MetadataErrorKind, NtError, Result};
use crate::fs::atomic_write;
use crate::note::{Date, NoteId, NoteKind, Priority, QualifiedCollection, Status};
use crate::repository::{NoteChange, Repository};

use super::{editor_temp_path, ensure_note_exists, validate_tag};

#[derive(Debug)]
enum UpdateOperation {
    Kind(Option<NoteKind>),
    Status(Option<Status>),
    Priority(Option<Priority>),
    Scheduled(Option<Date>),
    Due(Option<Date>),
    Home(QualifiedCollection),
    Tag {
        add: bool,
        value: String,
    },
    Collection {
        add: bool,
        value: QualifiedCollection,
    },
    Link {
        add: bool,
        value: NoteId,
    },
    Source {
        add: bool,
        value: String,
    },
}

#[derive(Clone, Copy)]
enum MetadataUpdateField {
    Value(ValueUpdateField),
    Set(SetUpdateField),
}

impl MetadataUpdateField {
    fn name(self) -> &'static str {
        match self {
            Self::Value(field) => field.name(),
            Self::Set(field) => field.name(),
        }
    }
}

impl UpdateOperation {
    fn parse_value(field: ValueUpdateField, raw: &str) -> Result<Self> {
        match field {
            ValueUpdateField::Kind => {
                Ok(Self::Kind((raw != "-").then(|| raw.parse()).transpose()?))
            }
            ValueUpdateField::Status => {
                Ok(Self::Status((raw != "-").then(|| raw.parse()).transpose()?))
            }
            ValueUpdateField::Priority => Ok(Self::Priority(
                (raw != "-").then(|| raw.parse()).transpose()?,
            )),
            ValueUpdateField::Scheduled => Ok(Self::Scheduled(
                (raw != "-").then(|| raw.parse()).transpose()?,
            )),
            ValueUpdateField::Due => Ok(Self::Due((raw != "-").then(|| raw.parse()).transpose()?)),
            ValueUpdateField::Home => Ok(Self::Home(raw.parse()?)),
        }
    }

    fn parse_set(field: SetUpdateField, raw: &str, repository: &Repository) -> Result<Self> {
        let (add, value) = raw
            .strip_prefix('+')
            .map(|value| (true, value))
            .or_else(|| raw.strip_prefix('-').map(|value| (false, value)))
            .ok_or_else(|| NtError::InvalidMetadata {
                command: "update",
                field: Some(field.name().to_string()),
                value: Some(raw.to_string()),
                kind: MetadataErrorKind::RequiresSignedValue,
            })?;
        if value.is_empty() {
            return Err(NtError::InvalidMetadata {
                command: "update",
                field: Some(field.name().to_string()),
                value: Some(raw.to_string()),
                kind: MetadataErrorKind::EmptyValue,
            });
        }

        Ok(match field {
            SetUpdateField::Tag => {
                validate_tag(value)?;
                Self::Tag {
                    add,
                    value: value.to_string(),
                }
            }
            SetUpdateField::Collection => Self::Collection {
                add,
                value: value.parse()?,
            },
            SetUpdateField::Link => {
                let id: NoteId = value.parse()?;
                ensure_note_exists(repository, &id)?;
                Self::Link { add, value: id }
            }
            SetUpdateField::Source => Self::Source {
                add,
                value: value.to_string(),
            },
        })
    }

    fn into_change(self) -> NoteChange {
        match self {
            Self::Kind(value) => NoteChange::Kind(value.unwrap_or(NoteKind::Note)),
            Self::Status(value) => NoteChange::Status(value),
            Self::Priority(value) => NoteChange::Priority(value),
            Self::Scheduled(value) => NoteChange::Scheduled(value),
            Self::Due(value) => NoteChange::Due(value),
            Self::Home(value) => NoteChange::Home(value),
            Self::Tag { add, value } => NoteChange::Tag { add, value },
            Self::Collection { add, value } => NoteChange::Collection { add, value },
            Self::Link { add, value } => NoteChange::Link { add, value },
            Self::Source { add, value } => NoteChange::Source { add, value },
        }
    }
}

pub(super) fn update(id: &str, field: UpdateField, value: Option<&str>) -> Result<()> {
    let id: NoteId = id.parse()?;
    let mut repository = Repository::open()?;

    match field {
        UpdateField::Body => update_body(&id, value, &mut repository),
        UpdateField::Value(field) => update_metadata(
            &id,
            MetadataUpdateField::Value(field),
            value,
            &mut repository,
        ),
        UpdateField::Set(field) => {
            update_metadata(&id, MetadataUpdateField::Set(field), value, &mut repository)
        }
    }
}

fn update_metadata(
    id: &NoteId,
    field: MetadataUpdateField,
    value: Option<&str>,
    repository: &mut Repository,
) -> Result<()> {
    super::ensure_note_exists(repository, id)?;
    let value = value.ok_or_else(|| NtError::InvalidMetadata {
        command: "update",
        field: Some(field.name().to_string()),
        value: None,
        kind: MetadataErrorKind::RequiresValue,
    })?;
    let operation = match field {
        MetadataUpdateField::Value(field) => UpdateOperation::parse_value(field, value)?,
        MetadataUpdateField::Set(field) => UpdateOperation::parse_set(field, value, repository)?,
    };
    let now = crate::note::timestamp_now();
    repository.update_note(id, &operation.into_change(), &now)?;
    println!("updated {id} {} {value}", field.name());
    Ok(())
}

fn update_body(id: &NoteId, value: Option<&str>, repository: &mut Repository) -> Result<()> {
    if value.is_some() {
        return Err(NtError::Message(
            "`body` update reads CommonMark from stdin or $EDITOR".to_string(),
        ));
    }
    let note = repository.get_note(id)?;
    let body = read_body(&note.body, id.as_str())?;
    let title = crate::note::title_from_body(&body)?;
    let now = crate::note::timestamp_now();
    repository.update_note_body(id, &note.updated, &note.body, &body, &title, &now)?;
    println!("updated {id} body");
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
