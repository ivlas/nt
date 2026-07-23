use crate::cli::UpdateField;
use crate::error::{NtError, Result};
use crate::index::{Index, NoteMeta};

use super::{
    apply_status_transition, ensure_note_exists, note_mut, push_unique_sorted, validate_collection,
    validate_kind, validate_priority, validate_status, validate_tag,
};

#[derive(Debug)]
enum UpdateOperation {
    Kind(Option<String>),
    Status(Option<String>),
    Priority(Option<String>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Set {
        field: UpdateField,
        add: bool,
        value: String,
    },
}

impl UpdateOperation {
    fn parse(field: UpdateField, raw: &str, index: &Index) -> Result<Self> {
        match field {
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
                        ensure_note_exists(index, value)?;
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

    fn apply(self, note: &mut NoteMeta, now: &str) {
        match self {
            Self::Kind(value) => {
                note.kind = value.unwrap_or_else(|| "note".to_string());
                if note.kind == "note" {
                    note.status = None;
                    note.priority = None;
                    note.scheduled = None;
                    note.due = None;
                    note.closed = None;
                }
            }
            Self::Status(value) => apply_status_transition(note, value, now),
            Self::Priority(value) => note.priority = value,
            Self::Scheduled(value) => note.scheduled = value,
            Self::Due(value) => note.due = value,
            Self::Set { field, add, value } => {
                let values = match field {
                    UpdateField::Tag => &mut note.tags,
                    UpdateField::Collection => &mut note.collections,
                    UpdateField::Link => &mut note.links,
                    UpdateField::Source => &mut note.sources,
                    _ => unreachable!(),
                };
                if add {
                    push_unique_sorted(values, value);
                } else {
                    values.retain(|item| item != &value);
                }
            }
        }
    }

    fn validate_for_note(&self, note: &NoteMeta) -> Result<()> {
        let field = match self {
            Self::Status(Some(_)) => Some("status"),
            Self::Priority(Some(_)) => Some("priority"),
            Self::Scheduled(Some(_)) => Some("scheduled"),
            Self::Due(Some(_)) => Some("due"),
            Self::Status(None) | Self::Priority(None) | Self::Scheduled(None) | Self::Due(None) => {
                None
            }
            Self::Kind(_) | Self::Set { .. } => None,
        };

        if let Some(field) = field
            && note.kind != "todo"
        {
            return Err(NtError::Message(format!(
                "`{field}` metadata is only valid for todo notes"
            )));
        }

        Ok(())
    }
}

fn field_name(field: UpdateField) -> &'static str {
    match field {
        UpdateField::Kind => "kind",
        UpdateField::Status => "status",
        UpdateField::Priority => "priority",
        UpdateField::Scheduled => "scheduled",
        UpdateField::Due => "due",
        UpdateField::Tag => "tag",
        UpdateField::Collection => "collection",
        UpdateField::Link => "link",
        UpdateField::Source => "source",
    }
}

pub(super) fn update(id: &str, field: UpdateField, value: &str) -> Result<()> {
    crate::note::validate_id(id)?;
    let mut index = Index::load()?;
    super::ensure_note_exists(&index, id)?;
    let operation = UpdateOperation::parse(field, value, &index)?;
    let now = crate::note::timestamp_now().iso;
    let note = note_mut(&mut index, id)?;
    operation.validate_for_note(note)?;
    operation.apply(note, &now);
    note.updated = now;
    index.save()?;
    println!("updated {id} {} {value}", field_name(field));
    Ok(())
}
