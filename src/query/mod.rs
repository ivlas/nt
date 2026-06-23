use std::collections::BTreeSet;

use crate::error::{NtError, Result};
use crate::index::{Index, NoteMeta};

mod eval;
mod parse;
mod plan;
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

#[derive(Debug)]
struct CandidateSet {
    ids: BTreeSet<String>,
    exact: bool,
}

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

        plan::intersect_candidates(candidates)
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

    fn matches(&self, index: &Index, note: &NoteMeta) -> Result<bool> {
        match self {
            Self::Bare(value) => {
                if index.metadata_term_matches(&note.id, value)
                    || eval::matches_metadata(note, value)
                {
                    Ok(true)
                } else {
                    eval::matches_body(index, note, value)
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
            Self::Body(value) => eval::matches_body(index, note, value),
            Self::Not(expr) => Ok(!expr.matches(index, note)?),
        }
    }

    fn candidate_ids(&self, index: &Index) -> Option<CandidateSet> {
        match self {
            Self::Bare(value) => plan::bare_word_candidates(index, value),
            Self::Id(value) => Some(plan::exact_candidate(
                index
                    .notes
                    .keys()
                    .filter(|id| parse::normalize(id).starts_with(value))
                    .cloned()
                    .collect(),
            )),
            Self::Tag(value) => Some(plan::exact_candidate(plan::ids_for_normalized_key(
                &index.tags,
                value,
            ))),
            Self::Title(_) => None,
            Self::Day(value) => Some(plan::exact_candidate(plan::ids_for_key(&index.days, value))),
            Self::Since(value) => Some(plan::exact_candidate(plan::ids_for_days(index, |day| {
                day >= value
            }))),
            Self::Before(value) => Some(plan::exact_candidate(plan::ids_for_days(index, |day| {
                day < value
            }))),
            Self::Kind(value) => Some(plan::exact_candidate(plan::ids_for_normalized_key(
                &index.kinds,
                value,
            ))),
            Self::Status(value) => Some(plan::exact_candidate(plan::ids_for_normalized_key(
                &index.statuses,
                value,
            ))),
            Self::Priority(_) | Self::Scheduled(_) | Self::Due(_) | Self::Closed(_) => None,
            Self::Collection(value) => Some(plan::exact_candidate(plan::ids_for_normalized_key(
                &index.collections,
                value,
            ))),
            Self::Link(value) => Some(plan::exact_candidate(plan::ids_for_normalized_key(
                &index.backlinks,
                value,
            ))),
            Self::Source(_) => None,
            Self::Body(value) => plan::body_candidates(index, value),
            Self::Not(expr) => {
                let candidate = expr.candidate_ids(index)?;
                if !candidate.exact {
                    return None;
                }

                let mut ids = plan::all_note_ids(index);
                for id in candidate.ids {
                    ids.remove(&id);
                }
                Some(plan::exact_candidate(ids))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use crate::index::{Index, NoteMeta, VaultMeta};

    use super::{Query, plan::intersect_candidates};

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
    fn indexed_body_entries_are_trusted_until_rebuild() {
        let mut index = active_index();
        let dir = temp_dir("query-body-stale-index");
        let path = dir.join("NT20260528T143012.md");
        fs::write(&path, "# Body\n\nFresh bodyonlyterm from disk.\n").unwrap();

        index.vaults.get_mut("notes").unwrap().path = dir.clone();
        let mut note = note("NT20260528T143012");
        note.path = path;
        index.upsert_note_with_body(note.clone(), "# Body\n\nOld indexed text.\n");

        let query = Query::parse(&["body:bodyonlyterm".to_string()]).unwrap();

        assert!(query.candidate_ids(&index).unwrap().is_empty());
        assert!(!query.matches(&index, &note).unwrap());

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
    fn not_body_candidates_are_verification_only_when_body_plan_is_superset() {
        let mut index = active_index();

        index.upsert_note_with_body(
            note("NT20260528T143012"),
            "# Indexed\n\nIndexed body text.\n",
        );
        index.upsert_note(note("NT20260529T120000"));

        let exact = Query::parse(&["not:body:indexed".to_string()]).unwrap();
        assert!(
            exact.candidate_ids(&index).is_none(),
            "unindexed active notes make body planning a superset, so not:body stays verification-only"
        );

        let fallback_only = Query::parse(&["not:body:fallback".to_string()]).unwrap();
        assert!(
            fallback_only.candidate_ids(&index).is_none(),
            "fallback-only body queries cannot be subtracted structurally"
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
    fn large_index_candidate_narrowing_is_structural_and_preserves_recent_order() {
        let mut index = active_index();
        let dir = temp_dir("query-large-index");
        index.vaults.get_mut("notes").unwrap().path = dir.clone();
        let mut expected_recent = Vec::new();

        for second in 0..1000 {
            let hour = second / 3600;
            let minute = second / 60 % 60;
            let second_of_minute = second % 60;
            let id = format!("NT20260601T{hour:02}{minute:02}{second_of_minute:02}");
            let mut item = note(&id);
            item.path = dir.join(format!("{id}.md"));
            item.created = format!("2026-06-01T{hour:02}:{minute:02}:{second_of_minute:02}Z");

            if matches!(second, 100 | 500 | 900) {
                item.tags = vec!["target".to_string()];
                item.status = Some("open".to_string());
                expected_recent.push(id.clone());
                fs::write(&item.path, "# Target\n\nneedle marker.\n").unwrap();
                index.upsert_note_with_body(item, "# Target\n\nneedle marker.\n");
            } else {
                if second % 2 == 0 {
                    item.tags = vec!["target".to_string()];
                }
                if second % 3 == 0 {
                    item.status = Some("open".to_string());
                }
                index.upsert_note_with_body(item, "# Filler\n\nhaystack marker.\n");
            }
        }

        expected_recent.sort_by(|left, right| right.cmp(left));

        let query = Query::parse(&[
            "tag:target".to_string(),
            "status:open".to_string(),
            "body:needle".to_string(),
        ])
        .unwrap();
        let candidates = query.candidate_ids(&index).unwrap();

        assert_eq!(candidates.len(), 3);
        assert_eq!(
            candidates,
            BTreeSet::from([
                "NT20260601T000140".to_string(),
                "NT20260601T000820".to_string(),
                "NT20260601T001500".to_string()
            ])
        );

        let found: Vec<&str> = index
            .active_recent_notes()
            .filter(|note| candidates.contains(&note.id))
            .filter(|note| query.matches(&index, note).unwrap())
            .map(|note| note.id.as_str())
            .collect();

        assert_eq!(found, expected_recent);

        let _ = fs::remove_dir_all(dir);
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
