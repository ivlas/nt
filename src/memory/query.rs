use crate::error::{NtError, Result};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MemoryListQuery {
    since: Option<i64>,
    until: Option<i64>,
    limit: Option<i64>,
}

impl MemoryListQuery {
    pub(crate) fn parse(expressions: &[String]) -> Result<Self> {
        let parsed = ParsedFilters::parse_only(expressions)?;
        Ok(Self {
            since: parsed.since,
            until: parsed.until,
            limit: parsed.limit,
        })
    }

    pub(crate) fn since(&self) -> Option<i64> {
        self.since
    }

    pub(crate) fn until(&self) -> Option<i64> {
        self.until
    }

    pub(crate) fn limit(&self) -> Option<i64> {
        self.limit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryRecallQuery {
    since: Option<i64>,
    until: Option<i64>,
    limit: Option<i64>,
    terms: Vec<String>,
}

impl MemoryRecallQuery {
    pub(crate) fn parse(expressions: &[String]) -> Result<Self> {
        let mut parsed = ParsedFilters::default();
        let mut terms = Vec::new();
        for expression in expressions {
            if parsed.parse_filter(expression)? {
                continue;
            }
            if is_filter_expression(expression) {
                return invalid("memory filter", expression);
            }
            terms.extend(literal_tokens(expression));
        }
        parsed.validate_range()?;
        sort_and_deduplicate(&mut terms);
        if terms.is_empty() {
            return Err(NtError::InvalidValue {
                field: "memory recall term",
                value: "none".to_string(),
            });
        }
        Ok(Self {
            since: parsed.since,
            until: parsed.until,
            limit: parsed.limit,
            terms,
        })
    }

    pub(crate) fn since(&self) -> Option<i64> {
        self.since
    }

    pub(crate) fn until(&self) -> Option<i64> {
        self.until
    }

    pub(crate) fn limit(&self) -> Option<i64> {
        self.limit
    }

    #[cfg(test)]
    pub(crate) fn terms(&self) -> &[String] {
        &self.terms
    }

    pub(crate) fn fts_expression(&self) -> String {
        fts_expression(&self.terms)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MemoryContextQuery {
    terms: Vec<String>,
}

impl MemoryContextQuery {
    pub(crate) fn parse(expressions: &[String]) -> Result<Self> {
        let mut terms = Vec::new();
        for expression in expressions {
            if is_filter_expression(expression) {
                return invalid("memory context filter", expression);
            }
            terms.extend(literal_tokens(expression));
        }
        sort_and_deduplicate(&mut terms);
        Ok(Self { terms })
    }

    pub(crate) fn terms(&self) -> &[String] {
        &self.terms
    }

    pub(crate) fn fts_expression(&self) -> String {
        fts_expression(&self.terms)
    }
}

pub(crate) fn fts_expression(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[derive(Default)]
struct ParsedFilters {
    since: Option<i64>,
    until: Option<i64>,
    limit: Option<i64>,
}

impl ParsedFilters {
    fn parse_only(expressions: &[String]) -> Result<Self> {
        let mut parsed = Self::default();
        for expression in expressions {
            if !parsed.parse_filter(expression)? {
                return invalid("memory filter", expression);
            }
        }
        parsed.validate_range()?;
        Ok(parsed)
    }

    fn parse_filter(&mut self, expression: &str) -> Result<bool> {
        if let Some(value) = expression.strip_prefix("since:") {
            parse_unique_positive("memory since", value, &mut self.since)?;
            return Ok(true);
        }
        if let Some(value) = expression.strip_prefix("until:") {
            parse_unique_positive("memory until", value, &mut self.until)?;
            return Ok(true);
        }
        if let Some(value) = expression.strip_prefix("limit:") {
            parse_unique_positive("memory limit", value, &mut self.limit)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn validate_range(&self) -> Result<()> {
        if self
            .since
            .zip(self.until)
            .is_some_and(|(since, until)| since > until)
        {
            return Err(NtError::InvalidValue {
                field: "memory range",
                value: "since exceeds until".to_string(),
            });
        }
        Ok(())
    }
}

fn parse_unique_positive(field: &'static str, value: &str, target: &mut Option<i64>) -> Result<()> {
    if target.is_some() {
        return Err(NtError::InvalidValue {
            field,
            value: "duplicate".to_string(),
        });
    }
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid(field, value);
    }
    let parsed = value.parse::<i64>().map_err(|_| NtError::InvalidValue {
        field,
        value: value.to_string(),
    })?;
    if parsed == 0 {
        return invalid(field, value);
    }
    *target = Some(parsed);
    Ok(())
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

fn sort_and_deduplicate(terms: &mut Vec<String>) {
    terms.sort();
    terms.dedup();
}

fn invalid<T>(field: &'static str, value: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{MemoryContextQuery, MemoryListQuery, MemoryRecallQuery, fts_expression};

    fn expressions(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn list_parses_positive_bounds_and_limit() {
        let query =
            MemoryListQuery::parse(&expressions(&["since:10", "until:20", "limit:5"])).unwrap();
        assert_eq!(query.since(), Some(10));
        assert_eq!(query.until(), Some(20));
        assert_eq!(query.limit(), Some(5));
        assert_eq!(MemoryListQuery::parse(&[]).unwrap().limit(), None);
    }

    #[test]
    fn filters_reject_unknown_duplicate_invalid_and_reversed_values() {
        for values in [
            vec!["tag:rust"],
            vec!["plain"],
            vec!["since:0"],
            vec!["until:-1"],
            vec!["limit:"],
            vec!["limit:9223372036854775808"],
            vec!["since:2", "since:3"],
            vec!["until:2", "until:3"],
            vec!["limit:2", "limit:3"],
            vec!["since:20", "until:10"],
        ] {
            assert!(MemoryListQuery::parse(&expressions(&values)).is_err());
        }
    }

    #[test]
    fn recall_tokenizes_unicode_and_deduplicates_deterministically() {
        let query = MemoryRecallQuery::parse(&expressions(&[
            "zebra café",
            "since:7",
            "café_alpha",
            "limit:8",
        ]))
        .unwrap();
        assert_eq!(query.since(), Some(7));
        assert_eq!(query.until(), None);
        assert_eq!(query.limit(), Some(8));
        assert_eq!(query.terms(), ["alpha", "café", "zebra"]);
        assert_eq!(
            query.fts_expression(),
            "\"alpha\" AND \"café\" AND \"zebra\""
        );
    }

    #[test]
    fn recall_requires_terms_and_rejects_unknown_filters() {
        for values in [vec![], vec!["***"], vec!["since:1"], vec!["source:book"]] {
            assert!(MemoryRecallQuery::parse(&expressions(&values)).is_err());
        }
    }

    #[test]
    fn context_permits_empty_terms_but_no_filters() {
        assert!(MemoryContextQuery::parse(&[]).unwrap().terms().is_empty());
        assert!(
            MemoryContextQuery::parse(&expressions(&["---"]))
                .unwrap()
                .terms()
                .is_empty()
        );
        let query = MemoryContextQuery::parse(&expressions(&["beta alpha", "beta"])).unwrap();
        assert_eq!(query.terms(), ["alpha", "beta"]);
        assert_eq!(query.fts_expression(), "\"alpha\" AND \"beta\"");
        assert!(MemoryContextQuery::parse(&expressions(&["since:1"])).is_err());
        assert!(MemoryContextQuery::parse(&expressions(&["kind:event"])).is_err());
    }

    #[test]
    fn fts_builder_quotes_and_escapes_every_literal() {
        assert_eq!(fts_expression(&[]), "");
        assert_eq!(
            fts_expression(&["a\"b".to_string(), "c".to_string()]),
            "\"a\"\"b\" AND \"c\""
        );
    }
}
