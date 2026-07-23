use crate::error::{NtError, Result};
use crate::index::NoteMeta;

mod eval;
mod parse;
mod suggest;

#[derive(Debug)]
pub struct Query {
    exprs: Vec<QueryExpr>,
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

    pub fn parse_list(exprs: &[String]) -> Result<Self> {
        let mut parsed = Vec::new();
        for expr in exprs {
            let parsed_expr = QueryExpr::parse(expr)?;
            if !parsed_expr.is_structured() {
                return Err(NtError::Message(format!(
                    "search expression `{expr}` is not supported by `nt list`; use `nt find`"
                )));
            }
            parsed.push(parsed_expr);
        }

        Ok(Self { exprs: parsed })
    }

    pub fn matches(&self, note: &NoteMeta) -> Result<bool> {
        for expr in &self.exprs {
            if !expr.matches(note)? {
                return Ok(false);
            }
        }

        Ok(true)
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

    fn matches(&self, note: &NoteMeta) -> Result<bool> {
        match self {
            Self::Bare(value) => {
                if eval::matches_metadata(note, value) {
                    Ok(true)
                } else {
                    eval::matches_body(note, value)
                }
            }
            Self::Id(value) => Ok(parse::normalize(&note.id).starts_with(value)),
            Self::Tag(value) => Ok(eval::contains_normalized(&note.tags, value)),
            Self::Title(value) => Ok(parse::normalize(&note.title).contains(value)),
            Self::Day(value) => Ok(note.created.get(0..10).is_some_and(|day| day == value)),
            Self::Since(value) => Ok(note
                .created
                .get(0..10)
                .is_some_and(|day| day >= value.as_str())),
            Self::Before(value) => Ok(note
                .created
                .get(0..10)
                .is_some_and(|day| day < value.as_str())),
            Self::Kind(value) => Ok(parse::normalize(&note.kind) == *value),
            Self::Status(value) => Ok(note
                .status
                .as_deref()
                .is_some_and(|status| parse::normalize(status) == *value)),
            Self::Priority(value) => Ok(note.priority.as_deref() == Some(value)),
            Self::Scheduled(value) => Ok(note.scheduled.as_deref() == Some(value)),
            Self::Due(value) => Ok(note.due.as_deref() == Some(value)),
            Self::Closed(value) => {
                Ok(note.closed.as_deref().and_then(|v| v.get(0..10)) == Some(value))
            }
            Self::Collection(value) => Ok(eval::contains_normalized(&note.collections, value)),
            Self::Link(value) => Ok(note
                .links
                .iter()
                .any(|link| parse::normalize(link) == *value)),
            Self::Source(value) => Ok(note
                .sources
                .iter()
                .any(|reference| parse::normalize(reference).contains(value))),
            Self::Body(value) => eval::matches_body(note, value),
            Self::Not(expr) => Ok(!expr.matches(note)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::index::NoteMeta;

    use super::Query;

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage Decision".to_string(),
        )
    }

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
        Query::parse_list(&["status:open".to_string(), "not:tag:draft".to_string()]).unwrap();
        Query::parse_list(&[]).unwrap();

        for expression in ["storage", "title:storage", "source:example", "body:storage"] {
            let error = Query::parse_list(&[expression.to_string()]).unwrap_err();
            assert!(error.to_string().contains("use `nt find`"));
        }
    }

    #[test]
    fn matches_metadata_fields_with_and_semantics() {
        let mut note = note("NT20260528T143012");
        note.kind = "todo".to_string();
        note.status = Some("open".to_string());
        note.collections = vec!["projects/nt".to_string()];
        note.sources = vec!["https://example.com/spec".to_string()];

        let query = Query::parse(&[
            "kind:todo".to_string(),
            "status:open".to_string(),
            "collection:projects/nt".to_string(),
            "source:example.com".to_string(),
            "since:2026-05-01".to_string(),
            "before:2026-06-01".to_string(),
        ])
        .unwrap();

        assert!(query.matches(&note).unwrap());
    }

    #[test]
    fn matches_link_direction() {
        let mut from = note("NT20260528T143012");
        let to = note("NT20260529T120000");
        from.links = vec![to.id.clone()];

        let link = Query::parse(&[format!("link:{}", to.id)]).unwrap();
        assert!(link.matches(&from).unwrap());
        assert!(!link.matches(&to).unwrap());
    }

    #[test]
    fn negates_simple_expressions() {
        let mut note = note("NT20260528T143012");
        note.tags = vec!["draft".to_string()];

        let query = Query::parse(&["not:tag:draft".to_string()]).unwrap();

        assert!(!query.matches(&note).unwrap());
    }

    #[test]
    fn matches_tag_shorthand_id_prefix_title_day_and_multiword_body() {
        let dir = temp_dir("query-multiword-body");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Storage Decision\n\nMicroVM jailer notes.\n").unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path;
        note.tags = vec!["QEMU".to_string()];

        let query = Query::parse(&[
            "#qemu".to_string(),
            "id:NT20260528".to_string(),
            "title:storage".to_string(),
            "day:2026-05-28".to_string(),
            "body:microvm jailer".to_string(),
        ])
        .unwrap();

        assert!(query.matches(&note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn multiword_body_values_match_all_terms_not_an_exact_phrase() {
        let dir = temp_dir("query-body-terms");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Body\n\nThe jailer starts the microvm.\n").unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path;

        let all_terms = Query::parse(&["body:microvm jailer".to_string()]).unwrap();
        assert!(all_terms.matches(&note).unwrap());

        let missing_term = Query::parse(&["body:microvm jailer missing".to_string()]).unwrap();
        assert!(!missing_term.matches(&note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bare_words_fall_back_to_body_search() {
        let dir = temp_dir("query-bare-body");
        let path = dir.join("NT20260528T143012.md");
        fs::write(
            &path,
            "# Storage Decision\n\nOnly the body has bodyonlyterm.\n",
        )
        .unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path;

        let query = Query::parse(&["bodyonlyterm".to_string()]).unwrap();

        assert!(query.matches(&note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn body_search_reads_current_file_contents() {
        let dir = temp_dir("query-fresh-body");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Body\n\nOld text.\n").unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path.clone();

        let query = Query::parse(&["body:fresh".to_string()]).unwrap();
        assert!(!query.matches(&note).unwrap());

        fs::write(&path, "# Body\n\nFresh text.\n").unwrap();
        assert!(query.matches(&note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn body_search_reports_missing_note_files() {
        let note = note("NT20260528T143012");

        let query = Query::parse(&["body:anything".to_string()]).unwrap();

        let error = query.matches(&note).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("note body not readable for NT20260528T143012")
        );
    }

    #[test]
    fn date_filters_include_since_and_exclude_before() {
        let note = note("NT20260528T143012");

        let matching = Query::parse(&[
            "since:2026-05-28".to_string(),
            "before:2026-05-29".to_string(),
        ])
        .unwrap();
        let too_late = Query::parse(&["before:2026-05-28".to_string()]).unwrap();

        assert!(matching.matches(&note).unwrap());
        assert!(!too_late.matches(&note).unwrap());
    }

    #[test]
    fn date_filters_accept_valid_leap_days() {
        let mut note = note("NT20240229T120000");
        note.created = "2024-02-29T12:00:00Z".to_string();

        let query = Query::parse(&["day:2024-02-29".to_string()]).unwrap();

        assert!(query.matches(&note).unwrap());
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
            Query::parse(&["id:bad".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `id` prefix `bad`; use a prefix of NTYYYYMMDDTHHmmss"
        );
        assert_eq!(
            Query::parse(&["link:NT20260528T14301".to_string()])
                .unwrap_err()
                .to_string(),
            "invalid `link` note id `nt20260528t14301`; use NTYYYYMMDDTHHmmss"
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
