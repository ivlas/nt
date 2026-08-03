use crate::error::{NtError, Result};
use crate::query::{ListFilter, Query};

mod field;
mod render;

pub use field::ListField;
pub use render::{render_row, render_table};

#[derive(Debug)]
pub struct ListRow {
    pub values: Vec<String>,
}

#[derive(Debug)]
pub struct ListRequest {
    pub fields: Vec<ListField>,
    pub filters: Vec<ListFilter>,
}

impl ListRequest {
    pub fn parse(args: &[String]) -> Result<Self> {
        if let Some(argument) = args.iter().find(|argument| argument.starts_with('-')) {
            return Err(NtError::Message(format!(
                "unexpected argument '{argument}'"
            )));
        }

        if let [projection, filters @ ..] = args
            && projection == "all"
        {
            return Self::notes(field::ALL_FIELDS.to_vec(), filters);
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
        Ok(Self {
            fields,
            filters: Query::parse_list_filters(filters)?,
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
        let request = ListRequest::parse(&args(&["id,title,status", "status:open"])).unwrap();
        assert_eq!(
            request.fields,
            vec![ListField::Id, ListField::Title, ListField::Status]
        );
    }

    #[test]
    fn default_and_filter_only_requests_use_summary_fields() {
        let request = ListRequest::parse(&[]).unwrap();
        assert_eq!(
            request.fields,
            vec![
                ListField::Id,
                ListField::Title,
                ListField::Kind,
                ListField::Status,
                ListField::Due,
                ListField::Tag,
            ]
        );

        let request = ListRequest::parse(&args(&["status:open"])).unwrap();
        assert_eq!(
            request.fields,
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
        let request = ListRequest::parse(&args(&["all", "status:open"])).unwrap();
        assert_eq!(request.fields.len(), 15);
    }

    #[test]
    fn set_valued_metadata_uses_regular_singular_fields() {
        let request =
            ListRequest::parse(&args(&["tag,collection,link,source", "tag:rust"])).unwrap();
        assert_eq!(
            request.fields,
            vec![
                ListField::Tag,
                ListField::Collection,
                ListField::Link,
                ListField::Source,
            ]
        );
    }

    #[test]
    fn rejects_invalid_field_lists() {
        for (value, expected) in [
            ("id,titel", "unknown list field `titel`"),
            ("id,,title", "empty list field"),
            ("id,id", "duplicate list field `id`"),
            ("tags", "unknown list field `tags`"),
            ("collections", "unknown list field `collections`"),
            ("sources", "unknown list field `sources`"),
            ("links", "unknown list field `links`"),
            ("ids", "unknown list field `ids`"),
            ("titles", "unknown list field `titles`"),
        ] {
            let error = ListRequest::parse(&args(&[value])).unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }
}
