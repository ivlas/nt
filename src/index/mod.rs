use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NtError, Result};
use crate::fs::{atomic_write, index_path};
use crate::note::validate_id;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Index {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub active_vault: Option<String>,
    #[serde(default)]
    pub vaults: BTreeMap<String, VaultMeta>,
    #[serde(default)]
    pub notes: BTreeMap<String, NoteMeta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VaultMeta {
    pub path: PathBuf,
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
    pub priority: Option<String>,
    #[serde(default)]
    pub scheduled: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub closed: Option<String>,
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
            active_vault: None,
            vaults: BTreeMap::new(),
            notes: BTreeMap::new(),
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
            priority: None,
            scheduled: None,
            due: None,
            closed: None,
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
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = index_path()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        atomic_write(&path, &bytes)
    }

    pub fn active_vault_path(&self) -> Option<&Path> {
        let active = self.active_vault.as_ref()?;
        self.vaults.get(active).map(|vault| vault.path.as_path())
    }

    pub fn note_is_in_active_vault(&self, note: &NoteMeta) -> bool {
        self.active_vault_path()
            .is_some_and(|path| note_is_in_vault_path(note, path))
    }

    pub fn active_notes(&self) -> Vec<&NoteMeta> {
        let mut notes: Vec<&NoteMeta> = self
            .notes
            .values()
            .filter(|note| self.note_is_in_active_vault(note))
            .collect();
        notes.sort_by(|left, right| {
            right
                .created
                .cmp(&left.created)
                .then_with(|| right.id.cmp(&left.id))
        });
        notes
    }

    pub fn upsert_note(&mut self, note: NoteMeta) {
        self.notes.insert(note.id.clone(), note);
    }

    pub fn remove_notes<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        let ids: BTreeSet<&str> = ids.into_iter().collect();
        for id in ids.iter().copied() {
            self.notes.remove(id);
        }
        for note in self.notes.values_mut() {
            note.links.retain(|link| !ids.contains(link.as_str()));
        }
    }

    pub fn create_vault_for_path(&mut self, path: PathBuf, created: String) -> Result<String> {
        let name = vault_name_from_path(&path);
        if self.vaults.contains_key(&name) {
            return Err(NtError::Message(format!(
                "vault `{name}` already exists; choose another notes directory name"
            )));
        }

        self.vaults
            .insert(name.clone(), VaultMeta { path, created });
        Ok(name)
    }
}

fn note_is_in_vault_path(note: &NoteMeta, vault_path: &Path) -> bool {
    validate_id(&note.id).is_ok() && note.path == vault_path.join(format!("{}.md", note.id))
}

fn vault_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("vault")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{Index, NoteMeta, VaultMeta};

    fn note(id: &str) -> NoteMeta {
        NoteMeta::new_note(
            id.to_string(),
            PathBuf::from(format!("notes/{id}.md")),
            "2026-05-28T14:30:12Z".to_string(),
            "2026-05-28T14:30:12Z".to_string(),
            "Storage".to_string(),
        )
    }

    fn active_index() -> Index {
        Index {
            active_vault: Some("notes".to_string()),
            vaults: BTreeMap::from([(
                "notes".to_string(),
                VaultMeta {
                    path: PathBuf::from("notes"),
                    created: "2026-05-28T14:30:12Z".to_string(),
                },
            )]),
            ..Default::default()
        }
    }

    #[test]
    fn active_notes_are_newest_first_and_scoped_to_the_active_vault() {
        let mut index = active_index();
        let mut older = note("NT20260527T120000");
        older.created = "2026-05-27T12:00:00Z".to_string();
        index.upsert_note(older);
        index.upsert_note(note("NT20260528T143012"));

        let mut other_vault = note("NT20260529T120000");
        other_vault.path = PathBuf::from("other/NT20260529T120000.md");
        index.upsert_note(other_vault);

        let ids: Vec<&str> = index
            .active_notes()
            .iter()
            .map(|note| note.id.as_str())
            .collect();

        assert_eq!(ids, vec!["NT20260528T143012", "NT20260527T120000"]);
    }

    #[test]
    fn removing_notes_cleans_inbound_links() {
        let mut index = Index::default();
        let mut first = note("NT20260528T143012");
        first.links = vec!["NT20260529T120000".to_string()];
        let second = note("NT20260529T120000");

        index.upsert_note(first);
        index.upsert_note(second);

        index.remove_notes(["NT20260529T120000"]);

        assert_eq!(index.notes["NT20260528T143012"].links, Vec::<String>::new());
    }

    #[test]
    fn deserializes_note_metadata_with_defaults() {
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
        assert_eq!(note.priority, None);
        assert_eq!(note.scheduled, None);
        assert_eq!(note.due, None);
        assert_eq!(note.closed, None);
        assert_eq!(note.tags, vec!["design".to_string()]);
        assert!(note.collections.is_empty());
        assert!(note.links.is_empty());
        assert_eq!(note.sources, vec!["https://example.com/spec".to_string()]);
    }

    #[test]
    fn index_round_trips_primary_metadata_only() {
        let mut index = active_index();
        let mut stored = note("NT20260528T143012");
        stored.tags = vec!["design".to_string()];
        index.upsert_note(stored);

        let bytes = serde_json::to_vec(&index).unwrap();
        let loaded: Index = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(loaded.active_vault.as_deref(), Some("notes"));
        assert_eq!(loaded.notes["NT20260528T143012"].tags, vec!["design"]);
    }
}
