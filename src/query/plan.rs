use std::collections::{BTreeMap, BTreeSet};

use crate::index::{Index, tokenize_text};

use super::CandidateSet;
use super::eval::matches_metadata;
use super::parse::normalize;
pub(super) fn exact_candidate(ids: BTreeSet<String>) -> CandidateSet {
    CandidateSet { ids, exact: true }
}

pub(super) fn superset_candidate(ids: BTreeSet<String>) -> CandidateSet {
    CandidateSet { ids, exact: false }
}

pub(super) fn intersect_candidates(
    mut candidates: Vec<BTreeSet<String>>,
) -> Option<BTreeSet<String>> {
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

pub(super) fn ids_for_key(index: &BTreeMap<String, Vec<String>>, key: &str) -> BTreeSet<String> {
    index
        .get(key)
        .into_iter()
        .flat_map(|ids| ids.iter().cloned())
        .collect()
}

pub(super) fn ids_for_normalized_key(
    index: &BTreeMap<String, Vec<String>>,
    key: &str,
) -> BTreeSet<String> {
    index
        .iter()
        .filter(|(indexed_key, _)| normalize(indexed_key) == key)
        .flat_map(|(_, ids)| ids.iter().cloned())
        .collect()
}

pub(super) fn ids_for_days(index: &Index, matches: impl Fn(&str) -> bool) -> BTreeSet<String> {
    index
        .days
        .iter()
        .filter(|(day, _)| matches(day))
        .flat_map(|(_, ids)| ids.iter().cloned())
        .collect()
}

pub(super) fn all_note_ids(index: &Index) -> BTreeSet<String> {
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

pub(super) fn body_candidates(index: &Index, value: &str) -> Option<CandidateSet> {
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

pub(super) fn bare_word_candidates(index: &Index, value: &str) -> Option<CandidateSet> {
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
