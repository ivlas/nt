use std::collections::BTreeSet;

use crate::cli::LinkDirection;
use crate::error::{NtError, Result};
use crate::fs::relative_to_cwd;
use crate::index::NoteMeta;
use crate::query::Query;

#[derive(Debug)]
pub enum ListRequest {
    Notes {
        fields: Vec<ListField>,
        query: Query,
    },
    Tags(Option<String>),
    Collections(Option<String>),
    Links {
        id: String,
        direction: Option<LinkDirection>,
    },
}

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

const ALL_FIELDS: &[ListField] = &[
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

const DEFAULT_FIELDS: &[ListField] = &[
    ListField::Id,
    ListField::Title,
    ListField::Kind,
    ListField::Status,
    ListField::Due,
    ListField::Tag,
];

impl ListRequest {
    pub fn parse(args: &[String]) -> Result<Self> {
        if let Some(argument) = args.iter().find(|argument| argument.starts_with('-')) {
            return Err(NtError::Message(format!(
                "unexpected argument '{argument}'"
            )));
        }

        match args {
            [mode, filters @ ..] if mode == "all" => {
                return Self::notes(ALL_FIELDS.to_vec(), filters);
            }
            [mode] if mode == "ids" => return Self::notes(vec![ListField::Id], &[]),
            [mode] if mode == "titles" => {
                return Self::notes(vec![ListField::Id, ListField::Title], &[]);
            }
            [mode] if mode == "tags" => return Ok(Self::Tags(None)),
            [mode, tag] if mode == "tags" => return Ok(Self::Tags(Some(tag.clone()))),
            [mode] if mode == "collections" => return Ok(Self::Collections(None)),
            [mode, collection] if mode == "collections" => {
                return Ok(Self::Collections(Some(collection.clone())));
            }
            [mode, id] if mode == "links" => {
                return Ok(Self::Links {
                    id: id.clone(),
                    direction: None,
                });
            }
            [mode, id, direction] if mode == "links" => {
                let direction = match direction.as_str() {
                    "from" => LinkDirection::From,
                    "to" => LinkDirection::To,
                    _ => {
                        return Err(NtError::Message(format!(
                            "invalid link direction `{direction}`; use `from` or `to`"
                        )));
                    }
                };
                return Ok(Self::Links {
                    id: id.clone(),
                    direction: Some(direction),
                });
            }
            [mode, ..]
                if matches!(
                    mode.as_str(),
                    "ids" | "titles" | "tags" | "collections" | "links"
                ) =>
            {
                return Err(NtError::Message(format!(
                    "invalid `nt list {mode}` arguments"
                )));
            }
            _ => {}
        }

        if args.is_empty() {
            return Self::notes(DEFAULT_FIELDS.to_vec(), &[]);
        }

        if is_filter(&args[0]) {
            return Self::notes(DEFAULT_FIELDS.to_vec(), args);
        }

        let fields = ListField::parse_list(&args[0])?;
        Self::notes(fields, &args[1..])
    }

    fn notes(fields: Vec<ListField>, filters: &[String]) -> Result<Self> {
        Ok(Self::Notes {
            fields,
            query: Query::parse_list(filters)?,
        })
    }
}

impl ListField {
    fn parse_list(value: &str) -> Result<Vec<Self>> {
        let mut fields = Vec::new();
        let mut seen = BTreeSet::new();

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

    fn render(self, note: &NoteMeta) -> String {
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
            Self::Tag => values(&note.tags),
            Self::Collection => values(&note.collections),
            Self::Link => values(&note.links),
            Self::Source => values(&note.sources),
        }
    }

    fn name(self) -> &'static str {
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

pub fn render_row(note: &NoteMeta, fields: &[ListField]) -> String {
    fields
        .iter()
        .map(|field| field.render(note))
        .collect::<Vec<_>>()
        .join("\t")
}

pub fn render_table(notes: &[&NoteMeta], fields: &[ListField]) -> Vec<String> {
    let headers = fields.iter().map(|field| field.name()).collect::<Vec<_>>();
    let rows = notes
        .iter()
        .map(|note| {
            fields
                .iter()
                .map(|field| field.render(note))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let widths = fields
        .iter()
        .enumerate()
        .map(|(column, _)| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .chain([headers[column].len()])
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    std::iter::once(format_columns(
        headers.iter().map(|header| (*header).to_string()),
        &widths,
    ))
    .chain(
        rows.into_iter()
            .map(|row| format_columns(row.into_iter(), &widths)),
    )
    .collect()
}

fn format_columns(values: impl Iterator<Item = String>, widths: &[usize]) -> String {
    let last = widths.len().saturating_sub(1);
    values
        .enumerate()
        .map(|(column, value)| {
            if column == last {
                value
            } else {
                let padding = widths[column].saturating_sub(value.chars().count()) + 2;
                format!("{value}{}", " ".repeat(padding))
            }
        })
        .collect()
}

fn is_filter(value: &str) -> bool {
    value.starts_with('#') || value.contains(':')
}

fn optional(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "-".to_string())
}

fn values(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::{ListField, ListRequest, render_table};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn table_has_headers_and_aligned_columns() {
        let mut short = NoteMeta::new_note(
            "NT20260621T100000".to_string(),
            PathBuf::from("NT20260621T100000.md"),
            "2026-06-21T10:00:00Z".to_string(),
            "2026-06-21T10:00:00Z".to_string(),
            "Short".to_string(),
        );
        short.status = Some("open".to_string());
        let long = NoteMeta::new_note(
            "NT20260621T110000".to_string(),
            PathBuf::from("NT20260621T110000.md"),
            "2026-06-21T11:00:00Z".to_string(),
            "2026-06-21T11:00:00Z".to_string(),
            "A much longer title".to_string(),
        );

        let lines = render_table(
            &[&short, &long],
            &[ListField::Id, ListField::Title, ListField::Status],
        );

        assert_eq!(lines[0], "id                 title                status");
        assert_eq!(lines[1], "NT20260621T100000  Short                open");
        assert_eq!(lines[2], "NT20260621T110000  A much longer title  -");
    }

    #[test]
    fn parses_fields_and_filters() {
        let ListRequest::Notes { fields, .. } =
            ListRequest::parse(&args(&["id,title,status", "status:open"])).unwrap()
        else {
            panic!("expected note listing");
        };
        assert_eq!(
            fields,
            vec![ListField::Id, ListField::Title, ListField::Status]
        );
    }

    #[test]
    fn default_and_filter_only_requests_use_summary_fields() {
        let ListRequest::Notes { fields, .. } = ListRequest::parse(&[]).unwrap() else {
            panic!("expected note listing");
        };
        assert_eq!(
            fields,
            vec![
                ListField::Id,
                ListField::Title,
                ListField::Kind,
                ListField::Status,
                ListField::Due,
                ListField::Tag,
            ]
        );

        let ListRequest::Notes { fields, .. } =
            ListRequest::parse(&args(&["status:open"])).unwrap()
        else {
            panic!("expected note listing");
        };
        assert_eq!(fields.len(), 6);
    }

    #[test]
    fn all_selects_every_field_and_accepts_filters() {
        let ListRequest::Notes { fields, .. } =
            ListRequest::parse(&args(&["all", "status:open"])).unwrap()
        else {
            panic!("expected note listing");
        };
        assert_eq!(fields.len(), 15);
    }

    #[test]
    fn rejects_invalid_field_lists() {
        for (value, expected) in [
            ("id,titel", "unknown list field `titel`"),
            ("id,,title", "empty list field"),
            ("id,id", "duplicate list field `id`"),
        ] {
            let error = ListRequest::parse(&args(&[value])).unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }
}
