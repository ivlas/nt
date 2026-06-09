use std::fs;

use crate::error::{NtError, Result};
use crate::index::NoteMeta;
use crate::note::validate_id;

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
    fn parse(expr: &str) -> Result<Self> {
        if let Some(tag) = expr.strip_prefix('#') {
            if tag.is_empty() {
                return Err(NtError::Message("empty tag query".to_string()));
            }
            return Ok(Self::Tag(normalize(tag)));
        }

        if let Some(inner) = expr.strip_prefix("not:") {
            if inner.is_empty() {
                return Err(NtError::Message("empty not query".to_string()));
            }
            return Ok(Self::Not(Box::new(Self::parse(inner)?)));
        }

        let Some((field, value)) = expr.split_once(':') else {
            return Ok(Self::Bare(normalize(expr)));
        };

        if value.is_empty() {
            return Err(NtError::Message(format!("empty query value for `{field}`")));
        }

        let value = normalize(value);
        match field {
            "id" => {
                validate_id_prefix(&value)?;
                Ok(Self::Id(value))
            }
            "tag" => Ok(Self::Tag(value)),
            "title" => Ok(Self::Title(value)),
            "day" => {
                validate_date_value(field, &value)?;
                Ok(Self::Day(value))
            }
            "since" => {
                validate_date_value(field, &value)?;
                Ok(Self::Since(value))
            }
            "before" => {
                validate_date_value(field, &value)?;
                Ok(Self::Before(value))
            }
            "kind" => Ok(Self::Kind(value)),
            "status" => Ok(Self::Status(value)),
            "collection" => Ok(Self::Collection(value)),
            "link" => {
                validate_note_id_value(field, &value)?;
                Ok(Self::Link(value))
            }
            "source" => Ok(Self::Source(value)),
            "body" => Ok(Self::Body(value)),
            _ => Err(NtError::Message(unknown_field_error(field))),
        }
    }

    fn matches(&self, note: &NoteMeta) -> Result<bool> {
        match self {
            Self::Bare(value) => {
                if matches_metadata(note, value) {
                    Ok(true)
                } else {
                    matches_body(note, value)
                }
            }
            Self::Id(value) => Ok(normalize(&note.id).starts_with(value)),
            Self::Tag(value) => Ok(contains_normalized(&note.tags, value)),
            Self::Title(value) => Ok(normalize(&note.title).contains(value)),
            Self::Day(value) => Ok(note.created.get(0..10).is_some_and(|day| day == value)),
            Self::Since(value) => Ok(note
                .created
                .get(0..10)
                .is_some_and(|day| day >= value.as_str())),
            Self::Before(value) => Ok(note
                .created
                .get(0..10)
                .is_some_and(|day| day < value.as_str())),
            Self::Kind(value) => Ok(normalize(&note.kind) == *value),
            Self::Status(value) => Ok(note
                .status
                .as_deref()
                .is_some_and(|status| normalize(status) == *value)),
            Self::Collection(value) => Ok(contains_normalized(&note.collections, value)),
            Self::Link(value) => Ok(note.links.iter().any(|link| normalize(link) == *value)),
            Self::Source(value) => Ok(note
                .sources
                .iter()
                .any(|reference| normalize(reference).contains(value))),
            Self::Body(value) => matches_body(note, value),
            Self::Not(expr) => Ok(!expr.matches(note)?),
        }
    }
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn validate_date_value(field: &str, value: &str) -> Result<()> {
    let valid_shape = value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit());

    if !valid_shape {
        return Err(NtError::Message(format!(
            "invalid `{field}` date `{value}`; use YYYY-MM-DD"
        )));
    }

    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);
    let year: u32 = value[0..4].parse().unwrap_or(0);
    let max_day = days_in_month(year, month);
    if max_day == 0 || day == 0 || day > max_day {
        return Err(NtError::Message(format!(
            "invalid `{field}` date `{value}`; use YYYY-MM-DD"
        )));
    }

    Ok(())
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year % 4 == 0 && year % 100 != 0 || year % 400 == 0
}

fn validate_note_id_value(field: &str, value: &str) -> Result<()> {
    validate_id(&value.to_ascii_uppercase()).map_err(|_| {
        NtError::Message(format!(
            "invalid `{field}` note id `{value}`; use NTYYYYMMDDTHHmmss"
        ))
    })
}

fn validate_id_prefix(value: &str) -> Result<()> {
    if value.len() > 17 || !value.starts_with("nt") {
        return Err(invalid_id_prefix(value));
    }

    for (index, byte) in value.bytes().enumerate() {
        let valid = match index {
            0 => byte == b'n',
            1 | 10 => byte == b't',
            2..=9 | 11..=16 => byte.is_ascii_digit(),
            _ => false,
        };
        if !valid {
            return Err(invalid_id_prefix(value));
        }
    }

    Ok(())
}

fn invalid_id_prefix(value: &str) -> NtError {
    NtError::Message(format!(
        "invalid `id` prefix `{value}`; use a prefix of NTYYYYMMDDTHHmmss"
    ))
}

fn contains_normalized(values: &[String], needle: &str) -> bool {
    values.iter().any(|value| normalize(value) == needle)
}

fn matches_metadata(note: &NoteMeta, needle: &str) -> bool {
    note.id.to_ascii_lowercase().contains(needle)
        || note.title.to_ascii_lowercase().contains(needle)
        || note.kind.to_ascii_lowercase().contains(needle)
        || note
            .status
            .as_deref()
            .is_some_and(|status| status.to_ascii_lowercase().contains(needle))
        || note
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(needle))
        || note
            .collections
            .iter()
            .any(|collection| collection.to_ascii_lowercase().contains(needle))
        || note
            .links
            .iter()
            .any(|link| link.to_ascii_lowercase().contains(needle))
        || note
            .sources
            .iter()
            .any(|reference| reference.to_ascii_lowercase().contains(needle))
}

fn matches_body(note: &NoteMeta, needle: &str) -> Result<bool> {
    let body = fs::read_to_string(&note.path).map_err(|err| {
        NtError::Message(format!(
            "note body not readable for {} at {}: {err}",
            note.id,
            note.path.display()
        ))
    })?;

    Ok(body.to_ascii_lowercase().contains(needle))
}

fn unknown_field_error(field: &str) -> String {
    match query_field_suggestion(field) {
        Some(suggestion) => {
            format!("unknown query field `{field}`; did you mean `{suggestion}`?")
        }
        None => format!("unknown query field `{field}`"),
    }
}

fn query_field_suggestion(field: &str) -> Option<&'static str> {
    QUERY_FIELDS
        .iter()
        .copied()
        .map(|known| (edit_distance(field, known), known))
        .filter(|(distance, _)| *distance <= 2)
        .min_by_key(|(distance, known)| (*distance, known.len()))
        .map(|(_, known)| known)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_byte) in left.bytes().enumerate() {
        current[0] = left_index + 1;

        for (right_index, right_byte) in right.bytes().enumerate() {
            let replace = previous[right_index] + usize::from(left_byte != right_byte);
            let insert = current[right_index] + 1;
            let delete = previous[right_index + 1] + 1;
            current[right_index + 1] = replace.min(insert).min(delete);
        }

        previous.clone_from(&current);
    }

    previous[right.len()]
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
    fn matches_metadata_fields_with_and_semantics() {
        let mut note = note("NT20260528T143012");
        note.kind = "decision".to_string();
        note.status = Some("open".to_string());
        note.collections = vec!["projects/nt".to_string()];
        note.sources = vec!["https://example.com/spec".to_string()];

        let query = Query::parse(&[
            "kind:decision".to_string(),
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
