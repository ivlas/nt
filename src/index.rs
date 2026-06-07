use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::fs::{atomic_write, index_path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Index {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub active_notes_dir: Option<PathBuf>,
    #[serde(default)]
    pub notebooks: BTreeMap<String, NotebookMeta>,
    #[serde(default)]
    pub notes: BTreeMap<String, NoteMeta>,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub kinds: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub statuses: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub tags: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub collections: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub days: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub backlinks: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub terms: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NotebookMeta {
    pub created: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoteMeta {
    pub id: String,
    pub path: PathBuf,
    pub created: String,
    pub updated: String,
    pub title: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub collections: Vec<String>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

fn default_kind() -> String {
    "note".to_string()
}

fn default_version() -> u8 {
    1
}

impl Default for Index {
    fn default() -> Self {
        Self {
            version: 1,
            active_notes_dir: None,
            notebooks: BTreeMap::new(),
            notes: BTreeMap::new(),
            recent: Vec::new(),
            kinds: BTreeMap::new(),
            statuses: BTreeMap::new(),
            tags: BTreeMap::new(),
            collections: BTreeMap::new(),
            days: BTreeMap::new(),
            backlinks: BTreeMap::new(),
            terms: BTreeMap::new(),
        }
    }
}

impl NoteMeta {
    pub fn new_note(
        id: String,
        path: PathBuf,
        created: String,
        updated: String,
        title: String,
    ) -> Self {
        Self {
            id,
            path,
            created,
            updated,
            title,
            kind: default_kind(),
            status: None,
            tags: Vec::new(),
            collections: Vec::new(),
            links: Vec::new(),
            sources: Vec::new(),
        }
    }
}

impl Index {
    pub fn load() -> Result<Self> {
        let path = index_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let bytes = fs::read(path)?;
        let index: Self = serde_json::from_slice(&bytes)?;
        Ok(index)
    }

    pub fn save(&self) -> Result<()> {
        let path = index_path()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        atomic_write(&path, &bytes)
    }

    pub fn active_notes_dir(&self) -> Option<&Path> {
        self.active_notes_dir.as_deref()
    }

    pub fn upsert_note(&mut self, note: NoteMeta) {
        let id = note.id.clone();
        self.notes.insert(id, note);
        self.rebuild_derived();
    }

    pub fn remove_note(&mut self, id: &str) {
        self.notes.remove(id);
        self.rebuild_derived();
    }

    pub fn rebuild_derived(&mut self) {
        self.rebuild_derived_with_body_terms(&BTreeMap::new());
    }

    pub fn rebuild_derived_with_body_terms(
        &mut self,
        body_terms: &BTreeMap<String, BTreeSet<String>>,
    ) {
        let mut notes: Vec<&NoteMeta> = self.notes.values().collect();
        notes.sort_by(|left, right| {
            right
                .created
                .cmp(&left.created)
                .then_with(|| right.id.cmp(&left.id))
        });

        self.recent = notes.iter().map(|note| note.id.clone()).collect();
        self.kinds.clear();
        self.statuses.clear();
        self.tags.clear();
        self.collections.clear();
        self.days.clear();
        self.backlinks.clear();
        self.terms.clear();

        for note in notes {
            if let Some(day) = note.created.get(0..10) {
                self.days
                    .entry(day.to_string())
                    .or_default()
                    .push(note.id.clone());
            }

            self.kinds
                .entry(note.kind.clone())
                .or_default()
                .push(note.id.clone());

            if let Some(status) = &note.status {
                self.statuses
                    .entry(status.clone())
                    .or_default()
                    .push(note.id.clone());
            }

            for tag in &note.tags {
                self.tags
                    .entry(tag.to_string())
                    .or_default()
                    .push(note.id.clone());
            }

            for collection in &note.collections {
                self.collections
                    .entry(collection.to_string())
                    .or_default()
                    .push(note.id.clone());
            }

            for link in &note.links {
                self.backlinks
                    .entry(link.to_string())
                    .or_default()
                    .push(note.id.clone());
            }

            for term in terms_for_note(note) {
                self.terms.entry(term).or_default().push(note.id.clone());
            }

            if let Some(terms) = body_terms.get(&note.id) {
                for term in terms {
                    self.terms
                        .entry(term.clone())
                        .or_default()
                        .push(note.id.clone());
                }
            }
        }
    }
}

fn terms_for_note(note: &NoteMeta) -> BTreeSet<String> {
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
    let mut term = String::new();

    for char in text.chars() {
        if char.is_ascii_alphanumeric() {
            term.push(char.to_ascii_lowercase());
            continue;
        }

        if !term.is_empty() {
            terms.insert(std::mem::take(&mut term));
        }
    }

    if !term.is_empty() {
        terms.insert(term);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Index, NoteMeta};

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage".to_string(),
        )
    }

    #[test]
    fn rebuilds_derived_maps() {
        let mut index = Index::default();
        let mut storage = note("NT20260528T143012");
        storage.kind = "decision".to_string();
        storage.status = Some("open".to_string());
        storage.tags = vec!["design".to_string()];
        storage.collections = vec!["projects/nt".to_string()];
        storage.links = vec!["NT20260527T120000".to_string()];
        storage.sources = vec!["https://example.com/spec".to_string()];

        index.upsert_note(storage);

        assert_eq!(index.recent, vec!["NT20260528T143012"]);
        assert_eq!(
            index.kinds.get("decision").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.statuses.get("open").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.tags.get("design").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.collections.get("projects/nt").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.days.get("2026-05-28").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.backlinks.get("NT20260527T120000").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.terms.get("storage").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.terms.get("projects").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
    }

    #[test]
    fn deserializes_old_note_metadata_with_defaults() {
        let json = r#"{
            "id": "NT20260528T143012",
            "path": "notes/NT20260528T143012.md",
            "created": "2026-05-28T14:30:12Z",
            "updated": "2026-05-28T14:30:12Z",
            "title": "Storage",
            "tags": ["design"],
            "sources": ["https://example.com/spec"]
        }"#;

        let note: NoteMeta = serde_json::from_str(json).unwrap();

        assert_eq!(note.kind, "note");
        assert_eq!(note.status, None);
        assert_eq!(note.tags, vec!["design".to_string()]);
        assert!(note.collections.is_empty());
        assert!(note.links.is_empty());
        assert_eq!(note.sources, vec!["https://example.com/spec".to_string()]);
    }
}
