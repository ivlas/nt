use crate::error::{NtError, Result};
use crate::note::validate_id;
use crate::query::Query;

mod field;
mod render;

pub use field::ListField;
pub use render::{render_link_row, render_link_table, render_row, render_table};

#[derive(Debug)]
pub enum ListRequest {
    Notes {
        fields: Vec<ListField>,
        query: Query,
    },
    Tags(Option<String>),
    Collections(Option<String>),
    Sources(Option<String>),
    LinkGraph {
        query: Query,
        from: Option<String>,
        to: Option<String>,
    },
}

impl ListRequest {
    pub fn parse(args: &[String]) -> Result<Self> {
        if let Some(argument) = args.iter().find(|argument| argument.starts_with('-')) {
            return Err(NtError::Message(format!(
                "unexpected argument '{argument}'"
            )));
        }

        if let [value, direction] = args
            && matches!(direction.as_str(), "from" | "to")
        {
            let id = value.strip_prefix("link:").unwrap_or(value);
            if validate_id(id).is_ok() {
                return Err(NtError::Message(format!(
                    "link direction `{direction}` must be an endpoint filter; use `nt list links {direction}:{id}`"
                )));
            }
        }

        match args {
            [mode, filters @ ..] if mode == "all" => {
                return Self::notes(field::ALL_FIELDS.to_vec(), filters);
            }
            [mode] if mode == "ids" => return Self::notes(vec![ListField::Id], &[]),
            [mode] if mode == "titles" => {
                return Self::notes(vec![ListField::Id, ListField::Title], &[]);
            }
            [mode, filters @ ..]
                if mode == "links" && filters.first().is_none_or(|value| is_filter(value)) =>
            {
                return Self::link_graph(filters);
            }
            [mode] if mode == "tags" => return Ok(Self::Tags(None)),
            [mode, tag] if mode == "tags" => return Ok(Self::Tags(Some(tag.clone()))),
            [mode] if mode == "collections" => return Ok(Self::Collections(None)),
            [mode, collection] if mode == "collections" => {
                return Ok(Self::Collections(Some(collection.clone())));
            }
            [mode] if mode == "sources" => return Ok(Self::Sources(None)),
            [mode, source] if mode == "sources" => {
                return Ok(Self::Sources(Some(source.clone())));
            }
            [mode, id] if mode == "links" => {
                validate_id(id)?;
                return Err(NtError::Message(format!(
                    "directionless link lookup is not supported; use `nt list links from:{id}` or `nt list links to:{id}`"
                )));
            }
            [mode, id, direction]
                if mode == "links" && matches!(direction.as_str(), "from" | "to") =>
            {
                validate_id(id)?;
                return Err(NtError::Message(format!(
                    "positional link directions are not supported; use `nt list links {direction}:{id}`"
                )));
            }
            [mode, ..]
                if matches!(
                    mode.as_str(),
                    "ids" | "titles" | "tags" | "collections" | "sources" | "links"
                ) =>
            {
                return Err(NtError::Message(format!(
                    "invalid `nt list {mode}` arguments"
                )));
            }
            _ => {}
        }

        if args.is_empty() {
            return Self::notes(field::DEFAULT_FIELDS.to_vec(), &[]);
        }

        if is_filter(&args[0]) {
            return Self::notes(field::DEFAULT_FIELDS.to_vec(), args);
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

    fn link_graph(filters: &[String]) -> Result<Self> {
        let mut from = None;
        let mut to = None;
        let mut note_filters = Vec::new();

        for filter in filters {
            let endpoint = filter
                .strip_prefix("from:")
                .map(|id| ("from", id, &mut from))
                .or_else(|| filter.strip_prefix("to:").map(|id| ("to", id, &mut to)));

            if let Some((name, id, selected)) = endpoint {
                validate_id(id)?;
                if selected.replace(id.to_string()).is_some() {
                    return Err(NtError::Message(format!(
                        "duplicate link endpoint filter `{name}`"
                    )));
                }
            } else {
                note_filters.push(filter.clone());
            }
        }

        Ok(Self::LinkGraph {
            query: Query::parse_list(&note_filters)?,
            from,
            to,
        })
    }
}

fn is_filter(value: &str) -> bool {
    value.starts_with('#') || value.contains(':')
}

#[cfg(test)]
mod tests {
    use super::{ListField, ListRequest};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
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
    fn sources_mode_parses_with_optional_filter() {
        assert!(matches!(
            ListRequest::parse(&args(&["sources"])).unwrap(),
            ListRequest::Sources(None)
        ));
        assert!(matches!(
            ListRequest::parse(&args(&["sources", "https://example.com"])).unwrap(),
            ListRequest::Sources(Some(value)) if value == "https://example.com"
        ));
    }

    #[test]
    fn links_without_an_id_selects_the_link_graph() {
        assert!(matches!(
            ListRequest::parse(&args(&["links"])).unwrap(),
            ListRequest::LinkGraph { .. }
        ));
        assert!(matches!(
            ListRequest::parse(&args(&["links", "day:2026-06-20"])).unwrap(),
            ListRequest::LinkGraph { .. }
        ));

        let ListRequest::LinkGraph { from, to, .. } = ListRequest::parse(&args(&[
            "links",
            "from:NT20260618T210731",
            "to:NT20260618T212305",
        ]))
        .unwrap() else {
            panic!("expected link graph");
        };
        assert_eq!(from.as_deref(), Some("NT20260618T210731"));
        assert_eq!(to.as_deref(), Some("NT20260618T212305"));
    }

    #[test]
    fn rejects_ambiguous_or_duplicate_link_endpoint_filters() {
        let error = ListRequest::parse(&args(&["links", "NT20260618T210731"])).unwrap_err();
        assert!(error.to_string().contains("directionless link lookup"));

        let error = ListRequest::parse(&args(&["links", "NT20260618T210731", "from"])).unwrap_err();
        assert_eq!(
            error.to_string(),
            "positional link directions are not supported; use `nt list links from:NT20260618T210731`"
        );

        let error = ListRequest::parse(&args(&[
            "links",
            "from:NT20260618T210731",
            "from:NT20260618T212305",
        ]))
        .unwrap_err();
        assert_eq!(error.to_string(), "duplicate link endpoint filter `from`");
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

    #[test]
    fn redirects_misplaced_link_directions() {
        for value in ["NT20260618T210731", "link:NT20260618T210731"] {
            let error = ListRequest::parse(&args(&[value, "from"])).unwrap_err();
            assert_eq!(
                error.to_string(),
                "link direction `from` must be an endpoint filter; use `nt list links from:NT20260618T210731`"
            );
        }
    }
}
