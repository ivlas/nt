use std::collections::{BTreeMap, BTreeSet};

use crate::index::{Index, NoteMeta};

impl Index {
    pub fn body_terms_match(&self, id: &str, terms: &[String]) -> Option<bool> {
        if terms.is_empty() || !self.body_indexed.iter().any(|indexed_id| indexed_id == id) {
            return None;
        }

        Some(terms.iter().all(|term| {
            self.body_terms
                .get(term)
                .is_some_and(|ids| ids.iter().any(|indexed_id| indexed_id == id))
        }))
    }

    pub fn metadata_term_matches(&self, id: &str, term: &str) -> bool {
        self.terms
            .get(term)
            .is_some_and(|ids| ids.iter().any(|indexed_id| indexed_id == id))
    }

    pub(super) fn refresh_text_terms(&mut self, id: &str, body: &str) {
        self.remove_text_terms(id);

        for term in tokenize_text(body) {
            push_indexed_id(self.body_terms.entry(term).or_default(), id);
        }

        for term in heading_terms_from_body(body) {
            push_indexed_id(self.heading_terms.entry(term).or_default(), id);
        }

        push_indexed_id(&mut self.body_indexed, id);
    }

    pub(super) fn remove_text_terms(&mut self, id: &str) {
        remove_indexed_id(&mut self.body_terms, id);
        remove_indexed_id(&mut self.heading_terms, id);
        self.body_indexed.retain(|indexed_id| indexed_id != id);
    }
}

pub(super) fn terms_for_note(note: &NoteMeta) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    insert_terms(&mut terms, &note.id);
    insert_terms(&mut terms, &note.title);
    insert_terms(&mut terms, &note.kind);

    if let Some(status) = &note.status {
        insert_terms(&mut terms, status);
    }

    for value in note
        .tags
        .iter()
        .chain(note.collections.iter())
        .chain(note.links.iter())
        .chain(note.sources.iter())
    {
        insert_terms(&mut terms, value);
    }

    terms
}

fn insert_terms(terms: &mut BTreeSet<String>, text: &str) {
    terms.extend(tokenize_text(text));
}

pub fn tokenize_text(text: &str) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    let mut term = String::new();

    for char in text.chars() {
        if char.is_alphanumeric() {
            term.extend(char.to_lowercase());
        } else if !term.is_empty() {
            terms.insert(std::mem::take(&mut term));
        }
    }

    if !term.is_empty() {
        terms.insert(term);
    }

    terms
}

fn heading_terms_from_body(body: &str) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();

    for line in body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }

        let heading = trimmed.trim_start_matches('#').trim();
        terms.extend(tokenize_text(heading));
    }

    terms
}

fn push_indexed_id(ids: &mut Vec<String>, id: &str) {
    if ids.iter().any(|existing| existing == id) {
        return;
    }

    ids.push(id.to_string());
    ids.sort();
}

fn remove_indexed_id(index: &mut BTreeMap<String, Vec<String>>, id: &str) {
    index.retain(|_, ids| {
        ids.retain(|indexed_id| indexed_id != id);
        !ids.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::tokenize_text;

    #[test]
    fn tokenizes_text_to_lowercase_unique_terms() {
        assert_eq!(
            tokenize_text("QEMU, qemu; Firecracker/v1"),
            BTreeSet::from([
                "firecracker".to_string(),
                "qemu".to_string(),
                "v1".to_string()
            ])
        );
    }
}
