use crate::error::{NtError, Result};
use crate::note::{CollectionPath, NoteId, Tag, Timestamp};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Filter {
    IdPrefix(String),
    Collection(CollectionPath),
    Tag(Tag),
    LinkedTo(NoteId),
    CreatedSince(Timestamp),
    UpdatedSince(Timestamp),
    Not(Box<Filter>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NoteQuery {
    filters: Vec<Filter>,
}

impl NoteQuery {
    pub fn parse_list(expressions: &[String]) -> Result<Self> {
        let filters = expressions
            .iter()
            .map(|expression| parse_filter(expression))
            .collect::<Result<_>>()?;
        Ok(Self { filters })
    }

    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }
}

fn parse_filter(expression: &str) -> Result<Filter> {
    if let Some(inner) = expression.strip_prefix("not:") {
        if inner.is_empty() {
            return invalid_filter(expression);
        }
        return Ok(Filter::Not(Box::new(parse_filter(inner)?)));
    }

    let Some((field, value)) = expression.split_once(':') else {
        return invalid_filter(expression);
    };
    if value.is_empty() {
        return invalid_filter(expression);
    }
    match field {
        "id" => Ok(Filter::IdPrefix(parse_id_prefix(value)?)),
        "collection" => Ok(Filter::Collection(value.parse()?)),
        "tag" => Ok(Filter::Tag(value.parse()?)),
        "link" => Ok(Filter::LinkedTo(value.parse()?)),
        "created-since" => Ok(Filter::CreatedSince(value.parse()?)),
        "updated-since" => Ok(Filter::UpdatedSince(value.parse()?)),
        _ => invalid_filter(expression),
    }
}

fn parse_id_prefix(value: &str) -> Result<String> {
    if value.len() > 36
        || value.bytes().enumerate().any(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte != b'-'
            } else {
                !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte)
            }
        })
    {
        return Err(NtError::InvalidValue {
            field: "id prefix",
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
}

fn invalid_filter<T>(expression: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field: "filter",
        value: expression.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Filter, NoteQuery};

    #[test]
    fn parses_structured_list_filters() {
        let query = NoteQuery::parse_list(&[
            "id:018fbe0a-6c00-7".to_string(),
            "collection:work/nt".to_string(),
            "not:tag:rust".to_string(),
        ])
        .unwrap();
        assert_eq!(query.filters().len(), 3);
        assert!(matches!(query.filters()[2], Filter::Not(_)));
    }

    #[test]
    fn rejects_bare_unknown_and_malformed_filters() {
        for value in ["rust", "kind:note", "id:", "id:ABC", "not:"] {
            assert!(NoteQuery::parse_list(&[value.to_string()]).is_err());
        }
    }
}
