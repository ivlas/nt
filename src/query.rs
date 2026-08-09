use crate::error::{NtError, Result};
use crate::note::{CollectionPath, NoteId, Tag, Timestamp};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Filter {
    IdPrefix(String),
    Collection(CollectionPath),
    Tag(Tag),
    LinksTo(NoteId),
    LinkedFrom(NoteId),
    CreatedSince(Timestamp),
    UpdatedSince(Timestamp),
    Not(Box<Filter>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NoteQuery {
    filters: Vec<Filter>,
    lexical_terms: Vec<String>,
    limit: Option<i64>,
}

impl NoteQuery {
    pub fn parse_list(expressions: &[String]) -> Result<Self> {
        let mut filters = Vec::new();
        let mut limit = None;
        for expression in expressions {
            if !parse_limit(expression, &mut limit)? {
                filters.push(parse_filter(expression)?);
            }
        }
        Ok(Self {
            filters,
            lexical_terms: Vec::new(),
            limit,
        })
    }

    pub fn parse_find(expressions: &[String]) -> Result<Self> {
        let mut filters = Vec::new();
        let mut lexical_terms = Vec::new();
        let mut limit = None;
        for expression in expressions {
            if parse_limit(expression, &mut limit)? {
                continue;
            } else if is_filter_expression(expression) {
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
            limit,
        })
    }

    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    pub fn lexical_terms(&self) -> &[String] {
        &self.lexical_terms
    }

    pub fn limit(&self) -> Option<i64> {
        self.limit
    }
}

fn parse_limit(expression: &str, limit: &mut Option<i64>) -> Result<bool> {
    let Some(value) = expression.strip_prefix("limit:") else {
        return Ok(false);
    };
    if limit.is_some() {
        return Err(NtError::InvalidValue {
            field: "limit",
            value: "duplicate".to_string(),
        });
    }
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid_limit(value);
    }
    let parsed = value.parse::<i64>().map_err(|_| NtError::InvalidValue {
        field: "limit",
        value: value.to_string(),
    })?;
    if parsed == 0 {
        return invalid_limit(value);
    }
    *limit = Some(parsed);
    Ok(true)
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
        "links-to" => Ok(Filter::LinksTo(value.parse()?)),
        "linked-from" => Ok(Filter::LinkedFrom(value.parse()?)),
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

fn invalid_limit<T>(value: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field: "limit",
        value: value.to_string(),
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
            "limit:50".to_string(),
        ])
        .unwrap();
        assert_eq!(query.filters().len(), 3);
        assert!(matches!(query.filters()[2], Filter::Not(_)));
        assert_eq!(query.limit(), Some(50));
    }

    #[test]
    fn parses_directional_link_filters_with_canonical_note_ids() {
        let id = "018fbe0a-6c00-7000-8000-000000000001";
        let query = NoteQuery::parse_list(&[format!("links-to:{id}"), format!("linked-from:{id}")])
            .unwrap();

        assert_eq!(
            query.filters(),
            [
                Filter::LinksTo(id.parse().unwrap()),
                Filter::LinkedFrom(id.parse().unwrap()),
            ]
        );
        assert!(NoteQuery::parse_list(&[format!("link:{id}")]).is_err());
        assert!(NoteQuery::parse_list(&[format!("incoming:{id}")]).is_err());
        assert!(NoteQuery::parse_list(&[format!("outgoing:{id}")]).is_err());
    }

    #[test]
    fn directional_link_filters_share_exact_note_id_validation() {
        for id in [
            "not-an-id",
            "018fbe0a-6c00-4000-8000-000000000001",
            "018FBE0A-6C00-7000-8000-000000000001",
        ] {
            assert!(NoteQuery::parse_list(&[format!("links-to:{id}")]).is_err());
            assert!(NoteQuery::parse_list(&[format!("linked-from:{id}")]).is_err());
        }
    }

    #[test]
    fn rejects_bare_unknown_and_malformed_filters() {
        for value in [
            "rust",
            "kind:note",
            "id:",
            "id:ABC",
            "id:0198abcd0",
            "id:0198abcd-0000-7000-8000-0000000000000",
            "not:",
        ] {
            assert!(NoteQuery::parse_list(&[value.to_string()]).is_err());
        }
    }

    #[test]
    fn find_combines_literal_tokens_and_structured_filters() {
        let query = NoteQuery::parse_find(&[
            "ownership-borrow".to_string(),
            "tag:rust".to_string(),
            "ownership".to_string(),
            "limit:25".to_string(),
        ])
        .unwrap();
        assert_eq!(query.filters().len(), 1);
        assert_eq!(query.lexical_terms(), ["borrow", "ownership"]);
        assert_eq!(query.limit(), Some(25));
    }

    #[test]
    fn queries_have_no_implicit_limit() {
        assert_eq!(NoteQuery::default().limit(), None);
        assert_eq!(NoteQuery::parse_list(&[]).unwrap().limit(), None);
        assert_eq!(
            NoteQuery::parse_find(&["rust".to_string()])
                .unwrap()
                .limit(),
            None
        );
    }

    #[test]
    fn rejects_invalid_and_duplicate_limits() {
        for value in [
            "limit:",
            "limit:0",
            "limit:-1",
            "limit:9223372036854775808",
            "limit:all",
        ] {
            assert!(NoteQuery::parse_list(&[value.to_string()]).is_err());
        }
        assert!(
            NoteQuery::parse_find(&[
                "rust".to_string(),
                "limit:10".to_string(),
                "limit:20".to_string(),
            ])
            .is_err()
        );
        assert!(NoteQuery::parse_list(&["not:limit:10".to_string()]).is_err());
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
