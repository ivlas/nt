use crate::error::{NtError, Result};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ListField {
    Id,
    Home,
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
    ListField::Home,
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
            "home" => Ok(Self::Home),
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

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Home => "home",
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
