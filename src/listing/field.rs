use crate::display::joined_or_dash;
use crate::error::{NtError, Result};
use crate::fs::relative_to_cwd;
use crate::index::NoteMeta;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ListField {
    Id,
    Path,
    Created,
    Updated,
    Title,
    Kind,
    Status,
    Priority,
    Scheduled,
    Due,
    Closed,
    Tag,
    Collection,
    Link,
    Source,
}

pub(super) const ALL_FIELDS: &[ListField] = &[
    ListField::Id,
    ListField::Path,
    ListField::Created,
    ListField::Updated,
    ListField::Title,
    ListField::Kind,
    ListField::Status,
    ListField::Priority,
    ListField::Scheduled,
    ListField::Due,
    ListField::Closed,
    ListField::Tag,
    ListField::Collection,
    ListField::Link,
    ListField::Source,
];

pub(super) const DEFAULT_FIELDS: &[ListField] = &[
    ListField::Id,
    ListField::Title,
    ListField::Kind,
    ListField::Status,
    ListField::Due,
    ListField::Tag,
];

impl ListField {
    pub(super) fn parse_list(value: &str) -> Result<Vec<Self>> {
        let mut fields = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for name in value.split(',') {
            if name.is_empty() {
                return Err(NtError::Message(format!("empty list field in `{value}`")));
            }
            let field = Self::parse(name)?;
            if !seen.insert(field) {
                return Err(NtError::Message(format!("duplicate list field `{name}`")));
            }
            fields.push(field);
        }

        Ok(fields)
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "id" => Ok(Self::Id),
            "path" => Ok(Self::Path),
            "created" => Ok(Self::Created),
            "updated" => Ok(Self::Updated),
            "title" => Ok(Self::Title),
            "kind" => Ok(Self::Kind),
            "status" => Ok(Self::Status),
            "priority" => Ok(Self::Priority),
            "scheduled" => Ok(Self::Scheduled),
            "due" => Ok(Self::Due),
            "closed" => Ok(Self::Closed),
            "tag" => Ok(Self::Tag),
            "collection" => Ok(Self::Collection),
            "link" => Ok(Self::Link),
            "source" => Ok(Self::Source),
            _ => Err(NtError::Message(format!("unknown list field `{value}`"))),
        }
    }

    pub(super) fn render(self, note: &NoteMeta) -> String {
        match self {
            Self::Id => note.id.clone(),
            Self::Path => relative_to_cwd(&note.path).display().to_string(),
            Self::Created => note.created.clone(),
            Self::Updated => note.updated.clone(),
            Self::Title => note.title.clone(),
            Self::Kind => note.kind.clone(),
            Self::Status => optional(&note.status),
            Self::Priority => optional(&note.priority),
            Self::Scheduled => optional(&note.scheduled),
            Self::Due => optional(&note.due),
            Self::Closed => optional(&note.closed),
            Self::Tag => joined_or_dash(&note.tags),
            Self::Collection => joined_or_dash(&note.collections),
            Self::Link => joined_or_dash(&note.links),
            Self::Source => joined_or_dash(&note.sources),
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Path => "path",
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Title => "title",
            Self::Kind => "kind",
            Self::Status => "status",
            Self::Priority => "priority",
            Self::Scheduled => "scheduled",
            Self::Due => "due",
            Self::Closed => "closed",
            Self::Tag => "tag",
            Self::Collection => "collection",
            Self::Link => "link",
            Self::Source => "source",
        }
    }
}

fn optional(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "-".to_string())
}
