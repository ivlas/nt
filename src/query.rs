use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::error::{NtError, Result};
use crate::index::{Index, NoteMeta, tokenize_text};
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

    pub fn matches(&self, index: &Index, note: &NoteMeta) -> Result<bool> {
        for expr in &self.exprs {
            if !expr.matches(index, note)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn candidate_ids(&self, index: &Index) -> Option<BTreeSet<String>> {
        let mut candidates = Vec::new();

        for expr in &self.exprs {
            if let Some(candidate) = expr.candidate_ids(index) {
                candidates.push(candidate.ids);
            }
        }

        intersect_candidates(candidates)
    }
}

#[derive(Debug)]
struct CandidateSet {
    ids: BTreeSet<String>,
    exact: bool,
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

    fn matches(&self, index: &Index, note: &NoteMeta) -> Result<bool> {
        match self {
            Self::Bare(value) => {
                if index.metadata_term_matches(&note.id, value) || matches_metadata(note, value) {
                    Ok(true)
                } else {
                    matches_body(index, note, value)
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
            Self::Body(value) => matches_body(index, note, value),
            Self::Not(expr) => Ok(!expr.matches(index, note)?),
        }
    }

    fn candidate_ids(&self, index: &Index) -> Option<CandidateSet> {
        match self {
            Self::Bare(value) => bare_word_candidates(index, value),
            Self::Id(value) => Some(exact_candidate(
                index
                    .notes
                    .keys()
                    .filter(|id| normalize(id).starts_with(value))
                    .cloned()
                    .collect(),
            )),
            Self::Tag(value) => Some(exact_candidate(ids_for_normalized_key(&index.tags, value))),
            Self::Title(_) => None,
            Self::Day(value) => Some(exact_candidate(ids_for_key(&index.days, value))),
            Self::Since(value) => Some(exact_candidate(ids_for_days(index, |day| day >= value))),
            Self::Before(value) => Some(exact_candidate(ids_for_days(index, |day| day < value))),
            Self::Kind(value) => Some(exact_candidate(ids_for_normalized_key(&index.kinds, value))),
            Self::Status(value) => Some(exact_candidate(ids_for_normalized_key(
                &index.statuses,
                value,
            ))),
            Self::Collection(value) => Some(exact_candidate(ids_for_normalized_key(
                &index.collections,
                value,
            ))),
            Self::Link(value) => Some(exact_candidate(ids_for_normalized_key(
                &index.backlinks,
                value,
            ))),
            Self::Source(_) => None,
            Self::Body(value) => body_candidates(index, value),
            Self::Not(expr) => {
                let candidate = expr.candidate_ids(index)?;
                if !candidate.exact {
                    return None;
                }

                let mut ids = all_note_ids(index);
                for id in candidate.ids {
                    ids.remove(&id);
                }
                Some(exact_candidate(ids))
            }
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

fn exact_candidate(ids: BTreeSet<String>) -> CandidateSet {
    CandidateSet { ids, exact: true }
}

fn superset_candidate(ids: BTreeSet<String>) -> CandidateSet {
    CandidateSet { ids, exact: false }
}

fn intersect_candidates(mut candidates: Vec<BTreeSet<String>>) -> Option<BTreeSet<String>> {
    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(BTreeSet::len);
    let mut intersected = candidates.remove(0);

    for candidate in candidates {
        if intersected.is_empty() {
            break;
        }
        intersected = intersected.intersection(&candidate).cloned().collect();
    }

    Some(intersected)
}

fn ids_for_key(index: &BTreeMap<String, Vec<String>>, key: &str) -> BTreeSet<String> {
    index
        .get(key)
        .into_iter()
        .flat_map(|ids| ids.iter().cloned())
        .collect()
}

fn ids_for_normalized_key(index: &BTreeMap<String, Vec<String>>, key: &str) -> BTreeSet<String> {
    index
        .iter()
        .filter(|(indexed_key, _)| normalize(indexed_key) == key)
        .flat_map(|(_, ids)| ids.iter().cloned())
        .collect()
}

fn ids_for_days(index: &Index, matches: impl Fn(&str) -> bool) -> BTreeSet<String> {
    index
        .days
        .iter()
        .filter(|(day, _)| matches(day))
        .flat_map(|(_, ids)| ids.iter().cloned())
        .collect()
}

fn all_note_ids(index: &Index) -> BTreeSet<String> {
    index.notes.keys().cloned().collect()
}

fn active_unindexed_body_ids(index: &Index) -> BTreeSet<String> {
    let indexed: BTreeSet<&str> = index.body_indexed.iter().map(String::as_str).collect();
    index
        .active_recent_notes()
        .filter(|note| !indexed.contains(note.id.as_str()))
        .map(|note| note.id.clone())
        .collect()
}

fn ids_with_all_terms(
    term_index: &BTreeMap<String, Vec<String>>,
    terms: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut candidates = Vec::new();
    for term in terms {
        candidates.push(ids_for_key(term_index, term));
    }

    intersect_candidates(candidates).unwrap_or_default()
}

fn body_candidates(index: &Index, value: &str) -> Option<CandidateSet> {
    let terms = tokenize_text(value);
    if terms.is_empty() {
        return None;
    }

    let mut ids = ids_with_all_terms(&index.body_terms, &terms);
    let unindexed = active_unindexed_body_ids(index);
    let exact = unindexed.is_empty();
    ids.extend(unindexed);

    Some(if exact {
        exact_candidate(ids)
    } else {
        superset_candidate(ids)
    })
}

fn bare_word_candidates(index: &Index, value: &str) -> Option<CandidateSet> {
    let terms = tokenize_text(value);
    if terms.is_empty() {
        return None;
    }

    let mut ids = ids_for_key(&index.terms, value);
    ids.extend(
        index
            .notes
            .values()
            .filter(|note| matches_metadata(note, value))
            .map(|note| note.id.clone()),
    );

    let body = body_candidates(index, value)?;
    let exact = body.exact;
    ids.extend(body.ids);

    Some(if exact {
        exact_candidate(ids)
    } else {
        superset_candidate(ids)
    })
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

fn matches_body(index: &Index, note: &NoteMeta, needle: &str) -> Result<bool> {
    let terms: Vec<String> = tokenize_text(needle).into_iter().collect();
    if let Some(matches) = index.body_terms_match(&note.id, &terms) {
        if matches && !note.path.exists() {
            read_body(note)?;
        }
        return Ok(matches);
    }

    let body = read_body(note)?;

    Ok(body.to_ascii_lowercase().contains(needle))
}

fn read_body(note: &NoteMeta) -> Result<String> {
    fs::read_to_string(&note.path).map_err(|err| {
        NtError::Message(format!(
            "note body not readable for {} at {}: {err}",
            note.id,
            note.path.display()
        ))
    })
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
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use crate::index::{Index, NoteMeta, VaultMeta};

    use super::{Query, intersect_candidates};

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage Decision".to_string(),
        )
    }

    fn active_index() -> Index {
        let mut index = Index::default();
        index.active_vault = Some("notes".to_string());
        index.vaults.insert(
            "notes".to_string(),
            VaultMeta {
                path: PathBuf::from("notes"),
                created: "2026-05-28T14:30:12Z".to_string(),
            },
        );
        index
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
        let mut index = Index::default();
        let mut note = note("NT20260528T143012");
        note.kind = "decision".to_string();
        note.status = Some("open".to_string());
        note.collections = vec!["projects/nt".to_string()];
        note.sources = vec!["https://example.com/spec".to_string()];
        index.upsert_note(note.clone());

        let query = Query::parse(&[
            "kind:decision".to_string(),
            "status:open".to_string(),
            "collection:projects/nt".to_string(),
            "source:example.com".to_string(),
            "since:2026-05-01".to_string(),
            "before:2026-06-01".to_string(),
        ])
        .unwrap();

        assert!(query.matches(&index, &note).unwrap());
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
        assert!(link.matches(&index, &from).unwrap());
        assert!(!link.matches(&index, &to).unwrap());
    }

    #[test]
    fn negates_simple_expressions() {
        let mut index = Index::default();
        let mut note = note("NT20260528T143012");
        note.tags = vec!["draft".to_string()];
        index.upsert_note(note.clone());

        let query = Query::parse(&["not:tag:draft".to_string()]).unwrap();

        assert!(!query.matches(&index, &note).unwrap());
    }

    #[test]
    fn matches_tag_shorthand_id_prefix_title_day_and_multiword_body() {
        let mut index = Index::default();
        let dir = temp_dir("query-multiword-body");
        let path = dir.join("NT20260528T143012.md");
        let body = "# Storage Decision\n\nMicroVM jailer notes.\n";
        fs::write(&path, body).unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path;
        note.tags = vec!["QEMU".to_string()];
        index.upsert_note_with_body(note.clone(), body);

        let query = Query::parse(&[
            "#qemu".to_string(),
            "id:NT20260528".to_string(),
            "title:storage".to_string(),
            "day:2026-05-28".to_string(),
            "body:microvm jailer".to_string(),
        ])
        .unwrap();

        assert!(query.matches(&index, &note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn plans_tag_status_and_body_intersection() {
        let mut index = active_index();
        let mut matching = note("NT20260528T143012");
        matching.tags = vec!["rust".to_string()];
        matching.status = Some("open".to_string());
        index.upsert_note_with_body(matching, "# Ownership\n\nBorrowed ownership notes.\n");

        let mut wrong_status = note("NT20260529T120000");
        wrong_status.created = "2026-05-29T12:00:00Z".to_string();
        wrong_status.tags = vec!["rust".to_string()];
        wrong_status.status = Some("done".to_string());
        index.upsert_note_with_body(wrong_status, "# Ownership\n\nOwnership notes.\n");

        let mut wrong_body = note("NT20260530T120000");
        wrong_body.created = "2026-05-30T12:00:00Z".to_string();
        wrong_body.tags = vec!["rust".to_string()];
        wrong_body.status = Some("open".to_string());
        index.upsert_note_with_body(wrong_body, "# Lifetimes\n\nRegion notes.\n");

        let query = Query::parse(&[
            "tag:rust".to_string(),
            "status:open".to_string(),
            "body:ownership".to_string(),
        ])
        .unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260528T143012".to_string()])
        );
    }

    #[test]
    fn plans_body_only_queries_from_body_terms() {
        let mut index = active_index();
        index.upsert_note_with_body(note("NT20260528T143012"), "# Body\n\nOwnership details.\n");
        index.upsert_note_with_body(note("NT20260529T120000"), "# Other\n\nLifetime details.\n");

        let query = Query::parse(&["body:ownership".to_string()]).unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260528T143012".to_string()])
        );
    }

    #[test]
    fn plans_bare_body_queries_from_body_terms() {
        let mut index = active_index();
        index.upsert_note_with_body(note("NT20260528T143012"), "# Body\n\nOwnership details.\n");
        index.upsert_note_with_body(note("NT20260529T120000"), "# Other\n\nLifetime details.\n");

        let query = Query::parse(&["ownership".to_string()]).unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260528T143012".to_string()])
        );
    }

    #[test]
    fn plans_empty_intersections_as_empty_candidates() {
        let mut index = active_index();
        let mut note = note("NT20260528T143012");
        note.tags = vec!["rust".to_string()];
        note.status = Some("open".to_string());
        index.upsert_note_with_body(note, "# Ownership\n\nOwnership details.\n");

        let query =
            Query::parse(&["status:done".to_string(), "body:ownership".to_string()]).unwrap();

        assert!(query.candidate_ids(&index).unwrap().is_empty());
    }

    #[test]
    fn body_candidates_keep_unindexed_notes_for_fallback() {
        let mut index = active_index();
        let dir = temp_dir("query-body-candidate-fallback");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Body\n\nOnly the body has bodyonlyterm.\n").unwrap();

        index.vaults.get_mut("notes").unwrap().path = dir.clone();
        let mut note = note("NT20260528T143012");
        note.path = path;
        index.upsert_note(note.clone());

        let query = Query::parse(&["body:bodyonlyterm".to_string()]).unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260528T143012".to_string()])
        );
        assert!(query.matches(&index, &note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn plans_not_candidates_only_when_inner_set_is_exact() {
        let mut index = active_index();
        let mut draft = note("NT20260528T143012");
        draft.tags = vec!["draft".to_string()];
        index.upsert_note(draft);
        index.upsert_note(note("NT20260529T120000"));

        let query = Query::parse(&["not:tag:draft".to_string()]).unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260529T120000".to_string()])
        );
    }

    #[test]
    fn plans_since_and_before_candidates() {
        let mut index = active_index();
        let mut before = note("NT20260527T120000");
        before.created = "2026-05-27T12:00:00Z".to_string();
        let matching = note("NT20260528T143012");
        let mut after = note("NT20260529T120000");
        after.created = "2026-05-29T12:00:00Z".to_string();
        index.upsert_note(before);
        index.upsert_note(matching);
        index.upsert_note(after);

        let query = Query::parse(&[
            "since:2026-05-28".to_string(),
            "before:2026-05-29".to_string(),
        ])
        .unwrap();

        assert_eq!(
            query.candidate_ids(&index).unwrap(),
            BTreeSet::from(["NT20260528T143012".to_string()])
        );
    }

    #[test]
    fn candidate_intersection_is_deterministic() {
        let candidates = intersect_candidates(vec![
            BTreeSet::from(["b".to_string(), "c".to_string()]),
            BTreeSet::from(["a".to_string(), "b".to_string(), "c".to_string()]),
            BTreeSet::from(["b".to_string()]),
        ]);

        assert_eq!(candidates.unwrap(), BTreeSet::from(["b".to_string()]));
    }

    #[test]
    fn bare_words_fall_back_to_body_search() {
        let index = Index::default();
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

        assert!(query.matches(&index, &note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bare_words_match_indexed_body_terms() {
        let mut index = Index::default();
        let dir = temp_dir("query-indexed-bare-body");
        let path = dir.join("NT20260528T143012.md");
        let body = "# Body\n\nOnly bodyonlyterm appears here.\n";
        fs::write(&path, body).unwrap();

        let mut note = note("NT20260528T143012");
        note.path = path;
        index.upsert_note_with_body(note.clone(), body);

        let query = Query::parse(&["bodyonlyterm".to_string()]).unwrap();

        assert!(query.matches(&index, &note).unwrap());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn date_filters_include_since_and_exclude_before() {
        let mut index = Index::default();
        let note = note("NT20260528T143012");
        index.upsert_note(note.clone());

        let matching = Query::parse(&[
            "since:2026-05-28".to_string(),
            "before:2026-05-29".to_string(),
        ])
        .unwrap();
        let too_late = Query::parse(&["before:2026-05-28".to_string()]).unwrap();

        assert!(matching.matches(&index, &note).unwrap());
        assert!(!too_late.matches(&index, &note).unwrap());
    }

    #[test]
    fn date_filters_accept_valid_leap_days() {
        let mut index = Index::default();
        let mut note = note("NT20240229T120000");
        note.created = "2024-02-29T12:00:00Z".to_string();
        index.upsert_note(note.clone());

        let query = Query::parse(&["day:2024-02-29".to_string()]).unwrap();

        assert!(query.matches(&index, &note).unwrap());
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
