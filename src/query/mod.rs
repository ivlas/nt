use crate::error::{NtError, Result};

mod eval;
mod parse;
mod suggest;

#[derive(Debug)]
pub struct Query {
    exprs: Vec<QueryExpr>,
}

pub(crate) struct SqlQuery {
    pub predicate: String,
    pub parameters: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum ListFilter {
    Id(String),
    Tag(String),
    Day(String),
    Since(String),
    Before(String),
    Kind(String),
    Status(String),
    Priority(String),
    Scheduled(String),
    Due(String),
    Closed(String),
    Collection(String),
    Link(String),
    Not(Box<ListFilter>),
}

#[derive(Debug)]
enum QueryExpr {
    Bare(String),
    Id(String),
    Tag(String),
    Title(String),
    Day(String),
    Since(String),
    Before(String),
    Kind(String),
    Status(String),
    Priority(String),
    Scheduled(String),
    Due(String),
    Closed(String),
    Collection(String),
    Link(String),
    Source(String),
    Body(String),
    Not(Box<QueryExpr>),
}

const QUERY_FIELDS: &[&str] = &[
    "id",
    "tag",
    "title",
    "day",
    "since",
    "before",
    "kind",
    "status",
    "priority",
    "scheduled",
    "due",
    "closed",
    "collection",
    "link",
    "source",
    "body",
];

impl Query {
    pub fn parse(exprs: &[String]) -> Result<Self> {
        if exprs.is_empty() {
            return Err(NtError::Message("usage: nt find <expr...>".to_string()));
        }

        let mut parsed = Vec::new();
        for expr in exprs {
            parsed.push(QueryExpr::parse(expr)?);
        }

        Ok(Self { exprs: parsed })
    }

    pub(crate) fn parse_list_filters(exprs: &[String]) -> Result<Vec<ListFilter>> {
        let mut parsed = Vec::new();
        for expr in exprs {
            let parsed_expr = QueryExpr::parse(expr)?;
            if !parsed_expr.is_structured() {
                return Err(NtError::Message(format!(
                    "search expression `{expr}` is not supported by `nt list`; use `nt find`"
                )));
            }
            parsed.push(parsed_expr.into_list_filter());
        }

        Ok(parsed)
    }

    pub(crate) fn sql(&self) -> SqlQuery {
        let mut predicate = String::new();
        let mut parameters = Vec::new();
        for (index, expr) in self.exprs.iter().enumerate() {
            if index > 0 {
                predicate.push_str(" AND ");
            }
            expr.push_sql(&mut predicate, &mut parameters);
        }
        SqlQuery {
            predicate,
            parameters,
        }
    }
}

impl QueryExpr {
    fn is_structured(&self) -> bool {
        match self {
            Self::Bare(_) | Self::Title(_) | Self::Source(_) | Self::Body(_) => false,
            Self::Not(expr) => expr.is_structured(),
            _ => true,
        }
    }

    fn parse(expr: &str) -> Result<Self> {
        if let Some(tag) = expr.strip_prefix('#') {
            if tag.is_empty() {
                return Err(NtError::Message("empty tag query".to_string()));
            }
            return Ok(Self::Tag(parse::normalize(tag)));
        }

        if let Some(inner) = expr.strip_prefix("not:") {
            if inner.is_empty() {
                return Err(NtError::Message("empty not query".to_string()));
            }
            return Ok(Self::Not(Box::new(Self::parse(inner)?)));
        }

        let Some((field, value)) = expr.split_once(':') else {
            return Ok(Self::Bare(parse::normalize(expr)));
        };

        if value.is_empty() {
            return Err(NtError::Message(format!("empty query value for `{field}`")));
        }

        let value = parse::normalize(value);
        match field {
            "id" => {
                parse::validate_id_prefix(&value)?;
                Ok(Self::Id(value))
            }
            "tag" => Ok(Self::Tag(value)),
            "title" => Ok(Self::Title(value)),
            "day" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Day(value))
            }
            "since" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Since(value))
            }
            "before" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Before(value))
            }
            "kind" => Ok(Self::Kind(value)),
            "status" => Ok(Self::Status(value)),
            "priority" => {
                parse::validate_priority(&value)?;
                Ok(Self::Priority(value.to_ascii_uppercase()))
            }
            "scheduled" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Scheduled(value))
            }
            "due" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Due(value))
            }
            "closed" => {
                parse::validate_date_value(field, &value)?;
                Ok(Self::Closed(value))
            }
            "collection" => Ok(Self::Collection(value)),
            "link" => {
                parse::validate_note_id_value(field, &value)?;
                Ok(Self::Link(value))
            }
            "source" => Ok(Self::Source(value)),
            "body" => Ok(Self::Body(value)),
            _ => Err(NtError::Message(parse::unknown_field_error(field))),
        }
    }

    fn into_list_filter(self) -> ListFilter {
        match self {
            Self::Id(value) => ListFilter::Id(value),
            Self::Tag(value) => ListFilter::Tag(value),
            Self::Day(value) => ListFilter::Day(value),
            Self::Since(value) => ListFilter::Since(value),
            Self::Before(value) => ListFilter::Before(value),
            Self::Kind(value) => ListFilter::Kind(value),
            Self::Status(value) => ListFilter::Status(value),
            Self::Priority(value) => ListFilter::Priority(value),
            Self::Scheduled(value) => ListFilter::Scheduled(value),
            Self::Due(value) => ListFilter::Due(value),
            Self::Closed(value) => ListFilter::Closed(value),
            Self::Collection(value) => ListFilter::Collection(value),
            Self::Link(value) => ListFilter::Link(value),
            Self::Not(expr) => ListFilter::Not(Box::new(expr.into_list_filter())),
            Self::Bare(_) | Self::Title(_) | Self::Source(_) | Self::Body(_) => {
                unreachable!("unstructured list filter")
            }
        }
    }

    fn push_sql(&self, sql: &mut String, parameters: &mut Vec<String>) {
        match self {
            Self::Bare(value) => push_bare_sql(sql, parameters, value),
            Self::Id(value) => {
                sql.push_str("n.id LIKE ?");
                parameters.push(format!("{value}%"));
            }
            Self::Tag(value) => {
                sql.push_str(
                    "n.id IN (SELECT filter_tags.note_id FROM note_tags filter_tags
                     WHERE lower(filter_tags.tag) = ?)",
                );
                parameters.push(value.clone());
            }
            Self::Title(value) => push_fts_sql(sql, parameters, value, Some("title")),
            Self::Day(value) => {
                sql.push_str("length(n.created) >= 10 AND substr(n.created, 1, 10) = ?");
                parameters.push(value.clone());
            }
            Self::Since(value) => {
                sql.push_str("length(n.created) >= 10 AND substr(n.created, 1, 10) >= ?");
                parameters.push(value.clone());
            }
            Self::Before(value) => {
                sql.push_str("length(n.created) >= 10 AND substr(n.created, 1, 10) < ?");
                parameters.push(value.clone());
            }
            Self::Kind(value) => {
                sql.push_str("lower(n.kind) = ?");
                parameters.push(value.clone());
            }
            Self::Status(value) => {
                sql.push_str("(n.status IS NOT NULL AND lower(n.status) = ?)");
                parameters.push(value.clone());
            }
            Self::Priority(value) => {
                sql.push_str("coalesce(n.priority = ?, 0)");
                parameters.push(value.clone());
            }
            Self::Scheduled(value) => {
                sql.push_str("coalesce(n.scheduled = ?, 0)");
                parameters.push(value.clone());
            }
            Self::Due(value) => {
                sql.push_str("coalesce(n.due = ?, 0)");
                parameters.push(value.clone());
            }
            Self::Closed(value) => {
                sql.push_str("coalesce(substr(n.closed, 1, 10) = ?, 0)");
                parameters.push(value.clone());
            }
            Self::Collection(value) => {
                sql.push_str(
                    "EXISTS (SELECT 1 FROM note_collections filter_nc
                     JOIN collections filter_c ON filter_c.id = filter_nc.collection_id
                     JOIN vaults filter_v ON filter_v.id = filter_c.vault_id
                     WHERE filter_nc.note_id = n.id
                       AND lower(filter_v.name || '/' || filter_c.name) = ?)",
                );
                parameters.push(value.clone());
            }
            Self::Link(value) => {
                sql.push_str(
                    "EXISTS (SELECT 1 FROM note_links filter_links
                     WHERE filter_links.note_id = n.id
                       AND lower(filter_links.target_id) = ?)",
                );
                parameters.push(value.clone());
            }
            Self::Source(value) => {
                sql.push_str(
                    "EXISTS (SELECT 1 FROM note_sources filter_sources
                     WHERE filter_sources.note_id = n.id AND ",
                );
                push_contains_sql(sql, parameters, "filter_sources.source", value);
                sql.push(')');
            }
            Self::Body(value) => push_fts_sql(sql, parameters, value, Some("body")),
            Self::Not(expr) => {
                sql.push_str("NOT (");
                expr.push_sql(sql, parameters);
                sql.push(')');
            }
        }
    }
}

fn push_bare_sql(sql: &mut String, parameters: &mut Vec<String>, value: &str) {
    sql.push('(');
    for (index, column) in [
        "n.id",
        "n.kind",
        "n.status",
        "n.priority",
        "n.scheduled",
        "n.due",
        "n.closed",
    ]
    .iter()
    .enumerate()
    {
        if index > 0 {
            sql.push_str(" OR ");
        }
        if matches!(*column, "n.scheduled" | "n.due") {
            push_case_sensitive_contains_sql(sql, parameters, column, value);
        } else {
            push_contains_sql(sql, parameters, column, value);
        }
    }
    for (table, column) in [
        ("note_tags", "tag"),
        ("note_links", "target_id"),
        ("note_sources", "source"),
    ] {
        sql.push_str(&format!(
            " OR EXISTS (SELECT 1 FROM {table} bare_values
             WHERE bare_values.note_id = n.id AND "
        ));
        push_contains_sql(sql, parameters, &format!("bare_values.{column}"), value);
        sql.push(')');
    }
    sql.push_str(
        " OR EXISTS (SELECT 1 FROM note_collections bare_nc
         JOIN collections bare_c ON bare_c.id = bare_nc.collection_id
         JOIN vaults bare_v ON bare_v.id = bare_c.vault_id
         WHERE bare_nc.note_id = n.id AND ",
    );
    push_contains_sql(sql, parameters, "bare_v.name || '/' || bare_c.name", value);
    sql.push_str(") OR ");
    push_fts_sql(sql, parameters, value, None);
    sql.push(')');
}

fn push_fts_sql(sql: &mut String, parameters: &mut Vec<String>, value: &str, column: Option<&str>) {
    let terms = eval::tokenize_text(value);
    if terms.is_empty() {
        sql.push('0');
        return;
    }

    sql.push_str(
        "n.id IN (SELECT search_rows.note_id FROM note_fts
         JOIN note_search_rows search_rows ON search_rows.search_id = note_fts.rowid
         WHERE note_fts MATCH ?)",
    );
    let terms = terms
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" AND ");
    parameters.push(match column {
        Some(column) => format!("{column} : ({terms})"),
        None => terms,
    });
}

fn push_contains_sql(
    sql: &mut String,
    parameters: &mut Vec<String>,
    expression: &str,
    value: &str,
) {
    sql.push_str("instr(lower(");
    sql.push_str(expression);
    sql.push_str("), ?) > 0");
    parameters.push(value.to_string());
}

fn push_case_sensitive_contains_sql(
    sql: &mut String,
    parameters: &mut Vec<String>,
    expression: &str,
    value: &str,
) {
    sql.push_str("instr(");
    sql.push_str(expression);
    sql.push_str(", ?) > 0");
    parameters.push(value.to_string());
}

#[cfg(test)]
mod tests {
    use super::Query;

    #[test]
    fn rejects_unknown_fields() {
        let err = Query::parse(&["collectiom:projects/nt".to_string()]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown query field `collectiom`; did you mean `collection`?"
        );
    }

    #[test]
    fn list_queries_accept_only_structured_filters() {
        Query::parse_list_filters(&["status:open".to_string(), "not:tag:draft".to_string()])
            .unwrap();
        Query::parse_list_filters(&[]).unwrap();

        for expression in ["storage", "title:storage", "source:example", "body:storage"] {
            let error = Query::parse_list_filters(&[expression.to_string()]).unwrap_err();
            assert!(error.to_string().contains("use `nt find`"));
        }
    }

    #[test]
    fn search_sql_binds_every_query_value() {
        let query = Query::parse(&[
            "id:018fbe0a".to_string(),
            "tag:bound-tag".to_string(),
            "title:bound-title".to_string(),
            "day:2026-01-02".to_string(),
            "since:2026-01-03".to_string(),
            "before:2026-01-04".to_string(),
            "kind:bound-kind".to_string(),
            "status:bound-status".to_string(),
            "priority:d".to_string(),
            "scheduled:2026-01-05".to_string(),
            "due:2026-01-06".to_string(),
            "closed:2026-01-07".to_string(),
            "collection:bound/collection".to_string(),
            "link:018fbe0a-6c00-7000-8000-000000000001".to_string(),
            "source:bound-source".to_string(),
            "body:boundbody".to_string(),
            "boundbare".to_string(),
            "not:tag:bound-negated".to_string(),
        ])
        .unwrap();

        let compiled = query.sql();
        assert_eq!(
            compiled.predicate.matches('?').count(),
            compiled.parameters.len()
        );
        for value in [
            "018fbe0a",
            "bound-tag",
            "2026-01-02",
            "2026-01-03",
            "2026-01-04",
            "bound-kind",
            "bound-status",
            "2026-01-05",
            "2026-01-06",
            "2026-01-07",
            "bound/collection",
            "018fbe0a-6c00-7000-8000-000000000001",
            "bound-source",
            "boundbody",
            "boundbare",
            "bound-negated",
        ] {
            assert!(!compiled.predicate.contains(value));
            assert!(
                compiled
                    .parameters
                    .iter()
                    .any(|parameter| parameter.contains(value))
            );
        }
        assert!(compiled.parameters.iter().any(|parameter| parameter == "D"));
        assert!(
            compiled
                .parameters
                .iter()
                .any(|parameter| parameter == "title : (\"bound\" AND \"title\")")
        );
    }

    #[test]
    fn rejects_invalid_typed_query_values() {
        assert_eq!(
            Query::parse(&["day:2026-99-01".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `day` date `2026-99-01`; use YYYY-MM-DD"
        );
        assert_eq!(
            Query::parse(&["day:2026-02-31".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `day` date `2026-02-31`; use YYYY-MM-DD"
        );
        assert_eq!(
            Query::parse(&["day:2025-02-29".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `day` date `2025-02-29`; use YYYY-MM-DD"
        );
        assert_eq!(
            Query::parse(&["id:xyz".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `id` prefix `xyz`; use a UUIDv7 prefix"
        );
        assert_eq!(
            Query::parse(&["link:018fbe0a".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `link` note id `018fbe0a`; use a UUIDv7"
        );
    }
}
