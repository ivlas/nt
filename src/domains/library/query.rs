use super::LibraryTimestamp;
use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryFilter {
    IdPrefix(String),
    Source(String),
    Title(String),
    CapturedSince(LibraryTimestamp),
    CapturedBefore(LibraryTimestamp),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryQuery {
    filters: Vec<LibraryFilter>,
    lexical_terms: Vec<String>,
    limit: Option<i64>,
}

impl LibraryQuery {
    pub fn parse_find(expressions: &[String]) -> Result<Self> {
        let mut filters = Vec::new();
        let mut lexical_terms = Vec::new();
        let mut limit = None;
        for expression in expressions {
            let Some((field, value)) = expression.split_once(':') else {
                lexical_terms.extend(literal_tokens(expression));
                continue;
            };
            if value.is_empty() {
                return invalid_filter(expression);
            }
            match field {
                "id" => filters.push(LibraryFilter::IdPrefix(parse_id_prefix(value)?)),
                "source" => filters.push(LibraryFilter::Source(value.to_string())),
                "title" => filters.push(LibraryFilter::Title(value.to_string())),
                "text" => lexical_terms.extend(literal_tokens(value)),
                "since" => filters.push(LibraryFilter::CapturedSince(value.parse()?)),
                "before" => filters.push(LibraryFilter::CapturedBefore(value.parse()?)),
                "limit" => {
                    if limit.is_some()
                        || value.is_empty()
                        || !value.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        return invalid_filter(expression);
                    }
                    let parsed = value.parse::<i64>().map_err(|_| NtError::InvalidValue {
                        field: "limit",
                        value: value.to_string(),
                    })?;
                    if parsed == 0 {
                        return invalid_filter(expression);
                    }
                    limit = Some(parsed);
                }
                _ => return invalid_filter(expression),
            }
        }
        lexical_terms.sort();
        lexical_terms.dedup();
        if lexical_terms.is_empty() {
            return Err(NtError::InvalidValue {
                field: "library search term",
                value: expressions.join(" "),
            });
        }
        Ok(Self {
            filters,
            lexical_terms,
            limit,
        })
    }

    pub fn filters(&self) -> &[LibraryFilter] {
        &self.filters
    }
    pub fn lexical_terms(&self) -> &[String] {
        &self.lexical_terms
    }
    pub fn limit(&self) -> Option<i64> {
        self.limit
    }
}

fn parse_id_prefix(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 36
        || value.bytes().enumerate().any(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte != b'-'
            } else {
                !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte)
            }
        })
    {
        return Err(NtError::InvalidValue {
            field: "library id prefix",
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
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

fn invalid_filter<T>(expression: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field: "library filter",
        value: expression.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal_and_structured_library_queries() {
        let query = LibraryQuery::parse_find(&[
            "write-ahead".to_string(),
            "source:https://sqlite.org/wal.html".to_string(),
            "since:2026-01-01T00:00:00Z".to_string(),
            "limit:20".to_string(),
        ])
        .unwrap();
        assert_eq!(query.lexical_terms(), ["ahead", "write"]);
        assert_eq!(query.filters().len(), 2);
        assert_eq!(query.limit(), Some(20));
    }

    #[test]
    fn rejects_unknown_filters_and_filter_only_searches() {
        assert!(LibraryQuery::parse_find(&["kind:web".to_string()]).is_err());
        assert!(LibraryQuery::parse_find(&["source:https://example.com".to_string()]).is_err());
    }
}
