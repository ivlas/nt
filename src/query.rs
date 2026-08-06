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
    lexical_terms: Vec<String>,
}

impl NoteQuery {
    pub fn parse_list(expressions: &[String]) -> Result<Self> {
        let filters = expressions
            .iter()
            .map(|expression| parse_filter(expression))
            .collect::<Result<_>>()?;
        Ok(Self {
            filters,
            lexical_terms: Vec::new(),
        })
    }

    pub fn parse_find(expressions: &[String]) -> Result<Self> {
        let mut filters = Vec::new();
        let mut lexical_terms = Vec::new();
        for expression in expressions {
            if is_filter_expression(expression) {
                filters.push(parse_filter(expression)?);
            } else {
                lexical_terms.extend(literal_tokens(expression));
            }
        }
        lexical_terms.sort();
        lexical_terms.dedup();
        if lexical_terms.is_empty() {
            return Err(NtError::InvalidValue {
                field: "search term",
                value: expressions.join(" "),
            });
        }
        Ok(Self {
            filters,
            lexical_terms,
        })
    }

    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    pub fn lexical_terms(&self) -> &[String] {
        &self.lexical_terms
    }
}

fn is_filter_expression(expression: &str) -> bool {
    let Some((field, _)) = expression.split_once(':') else {
        return false;
    };
    !field.is_empty()
        && field
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
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

fn literal_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character.is_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(start) = start.take() {
            tokens.push(value[start..index].to_string());
        }
    }
    if let Some(start) = start {
        tokens.push(value[start..].to_string());
    }
    tokens
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

    #[test]
    fn find_combines_literal_tokens_and_structured_filters() {
        let query = NoteQuery::parse_find(&[
            "ownership-borrow".to_string(),
            "tag:rust".to_string(),
            "ownership".to_string(),
        ])
        .unwrap();
        assert_eq!(query.filters().len(), 1);
        assert_eq!(query.lexical_terms(), ["borrow", "ownership"]);
    }

    #[test]
    fn find_rejects_raw_fields_negated_terms_and_empty_tokens() {
        for expressions in [
            vec!["title:storage"],
            vec!["not:storage"],
            vec!["***"],
            vec!["tag:rust"],
        ] {
            let expressions = expressions
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            assert!(NoteQuery::parse_find(&expressions).is_err());
        }
    }
}
