use std::fs;

use crate::error::{NtError, Result};
use crate::index::{Index, NoteMeta};

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

    pub fn matches(&self, index: &Index, note: &NoteMeta) -> bool {
        self.exprs.iter().all(|expr| expr.matches(index, note))
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
            "id" => Ok(Self::Id(value)),
            "tag" => Ok(Self::Tag(value)),
            "title" => Ok(Self::Title(value)),
            "day" => Ok(Self::Day(value)),
            "since" => Ok(Self::Since(value)),
            "before" => Ok(Self::Before(value)),
            "kind" => Ok(Self::Kind(value)),
            "status" => Ok(Self::Status(value)),
            "collection" => Ok(Self::Collection(value)),
            "link" => Ok(Self::Link(value)),
            "source" => Ok(Self::Source(value)),
            "body" => Ok(Self::Body(value)),
            _ => Err(NtError::Message(unknown_field_error(field))),
        }
    }

    fn matches(&self, index: &Index, note: &NoteMeta) -> bool {
        match self {
            Self::Bare(value) => matches_metadata(note, value) || matches_body(note, value),
            Self::Id(value) => normalize(&note.id).starts_with(value),
            Self::Tag(value) => contains_normalized(&note.tags, value),
            Self::Title(value) => normalize(&note.title).contains(value),
            Self::Day(value) => note.created.get(0..10).is_some_and(|day| day == value),
            Self::Since(value) => note
                .created
                .get(0..10)
                .is_some_and(|day| day >= value.as_str()),
            Self::Before(value) => note
                .created
                .get(0..10)
                .is_some_and(|day| day < value.as_str()),
            Self::Kind(value) => normalize(&note.kind) == *value,
            Self::Status(value) => note
                .status
                .as_deref()
                .is_some_and(|status| normalize(status) == *value),
            Self::Collection(value) => contains_normalized(&note.collections, value),
            Self::Link(value) => note.links.iter().any(|link| normalize(link) == *value),
            Self::Source(value) => note
                .sources
                .iter()
                .any(|reference| normalize(reference).contains(value)),
            Self::Body(value) => matches_body(note, value),
            Self::Not(expr) => !expr.matches(index, note),
        }
    }
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase()
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

fn matches_body(note: &NoteMeta, needle: &str) -> bool {
    let Ok(body) = fs::read_to_string(&note.path) else {
        return false;
    };

    body.to_ascii_lowercase().contains(needle)
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

    use crate::index::{Index, NoteMeta};

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
        let index = Index::default();
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

        assert!(query.matches(&index, &note));
    }

    #[test]
    fn matches_link_direction() {
        let mut index = Index::default();
        let mut from = note("NT20260528T143012");
        let to = note("NT20260529T120000");
        from.links = vec![to.id.clone()];
        index.upsert_note(from.clone());
        index.upsert_note(to.clone());

        let link = Query::parse(&[format!("link:{}", to.id)]).unwrap();
        assert!(link.matches(&index, &from));
        assert!(!link.matches(&index, &to));
    }

    #[test]
    fn negates_simple_expressions() {
        let index = Index::default();
        let mut note = note("NT20260528T143012");
        note.tags = vec!["draft".to_string()];

        let query = Query::parse(&["not:tag:draft".to_string()]).unwrap();

        assert!(!query.matches(&index, &note));
    }

    #[test]
    fn matches_tag_shorthand_id_prefix_title_day_and_multiword_body() {
        let dir = temp_dir("query-multiword-body");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Storage Decision\n\nMicroVM jailer notes.\n").unwrap();

        let index = Index::default();
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

        assert!(query.matches(&index, &note));

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

        let index = Index::default();
        let mut note = note("NT20260528T143012");
        note.path = path;

        let query = Query::parse(&["bodyonlyterm".to_string()]).unwrap();

        assert!(query.matches(&index, &note));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn date_filters_include_since_and_exclude_before() {
        let index = Index::default();
        let note = note("NT20260528T143012");

        let matching = Query::parse(&[
            "since:2026-05-28".to_string(),
            "before:2026-05-29".to_string(),
        ])
        .unwrap();
        let too_late = Query::parse(&["before:2026-05-28".to_string()]).unwrap();

        assert!(matching.matches(&index, &note));
        assert!(!too_late.matches(&index, &note));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nt-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
