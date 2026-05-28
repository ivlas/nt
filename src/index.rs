use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::fs::{atomic_write, index_path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Index {
    pub version: u8,
    pub active_notes_dir: Option<PathBuf>,
    pub notebooks: BTreeMap<String, NotebookMeta>,
    pub notes: BTreeMap<String, NoteMeta>,
    pub recent: Vec<String>,
    pub tags: BTreeMap<String, Vec<String>>,
    pub days: BTreeMap<String, Vec<String>>,
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
    pub tags: Vec<String>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            version: 1,
            active_notes_dir: None,
            notebooks: BTreeMap::new(),
            notes: BTreeMap::new(),
            recent: Vec::new(),
            tags: BTreeMap::new(),
            days: BTreeMap::new(),
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
        let mut notes: Vec<&NoteMeta> = self.notes.values().collect();
        notes.sort_by(|left, right| {
            right
                .created
                .cmp(&left.created)
                .then_with(|| right.id.cmp(&left.id))
        });

        self.recent = notes.iter().map(|note| note.id.clone()).collect();
        self.tags.clear();
        self.days.clear();

        for note in notes {
            if let Some(day) = note.created.get(0..10) {
                self.days
                    .entry(day.to_string())
                    .or_default()
                    .push(note.id.clone());
            }

            for tag in &note.tags {
                self.tags
                    .entry(tag.to_string())
                    .or_default()
                    .push(note.id.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Index, NoteMeta};

    #[test]
    fn rebuilds_recent_tags_and_days() {
        let mut index = Index::default();
        index.upsert_note(NoteMeta {
            id: "NT20260528T143012".to_string(),
            path: PathBuf::from("notes/NT20260528T143012.md"),
            created: "2026-05-28T14:30:12Z".to_string(),
            updated: "2026-05-28T14:30:12Z".to_string(),
            title: "Storage".to_string(),
            tags: vec!["design".to_string()],
        });

        assert_eq!(index.recent, vec!["NT20260528T143012"]);
        assert_eq!(
            index.tags.get("design").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.days.get("2026-05-28").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
    }
}
