use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NtError, Result};
use crate::fs::{atomic_write, index_path};
use crate::note::validate_id;

mod terms;

pub use terms::tokenize_text;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Index {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub active_vault: Option<String>,
    #[serde(default)]
    pub vaults: BTreeMap<String, VaultMeta>,
    #[serde(default, rename = "active_notes_dir", skip_serializing)]
    legacy_active_notes_dir: Option<PathBuf>,
    #[serde(default, rename = "notebooks", skip_serializing)]
    legacy_vaults: BTreeMap<String, LegacyVaultMeta>,
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
    #[serde(default)]
    pub body_terms: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub heading_terms: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub body_indexed: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VaultMeta {
    pub path: PathBuf,
    pub created: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LegacyVaultMeta {
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
            legacy_active_notes_dir: None,
            legacy_vaults: BTreeMap::new(),
            notes: BTreeMap::new(),
            recent: Vec::new(),
            kinds: BTreeMap::new(),
            statuses: BTreeMap::new(),
            tags: BTreeMap::new(),
            collections: BTreeMap::new(),
            days: BTreeMap::new(),
            backlinks: BTreeMap::new(),
            terms: BTreeMap::new(),
            body_terms: BTreeMap::new(),
            heading_terms: BTreeMap::new(),
            body_indexed: Vec::new(),
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
        let mut index: Self = serde_json::from_slice(&bytes)?;
        index.normalize_loaded();
        Ok(index)
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

    pub fn active_recent_notes(&self) -> impl Iterator<Item = &NoteMeta> {
        self.recent
            .iter()
            .filter_map(|id| self.notes.get(id))
            .filter(|note| self.note_is_in_active_vault(note))
    }

    #[cfg(test)]
    pub fn upsert_note(&mut self, note: NoteMeta) {
        let id = note.id.clone();
        self.notes.insert(id, note);
        self.rebuild_derived();
    }

    pub fn upsert_note_with_body(&mut self, note: NoteMeta, body: &str) {
        let id = note.id.clone();
        self.refresh_text_terms(&id, body);
        self.notes.insert(id, note);
        self.rebuild_derived();
    }

    pub fn remove_notes<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        let ids: BTreeSet<&str> = ids.into_iter().collect();
        for id in ids.iter().copied() {
            self.notes.remove(id);
            self.remove_text_terms(id);
        }
        for note in self.notes.values_mut() {
            note.links.retain(|link| !ids.contains(link.as_str()));
        }
        self.rebuild_derived();
    }

    pub fn replace_active_vault_notes_with_bodies(
        &mut self,
        notes: BTreeMap<String, NoteMeta>,
        bodies: &BTreeMap<String, String>,
    ) -> Result<()> {
        let active_vault = self
            .active_vault_path()
            .map(Path::to_path_buf)
            .ok_or(NtError::MissingVault)?;

        for (id, note) in &notes {
            if id != &note.id || !note_is_in_vault_path(note, &active_vault) {
                return Err(NtError::Message(format!(
                    "invalid rebuilt note path for `{id}`"
                )));
            }
            if let Some(existing) = self.notes.get(id)
                && existing.path != note.path
            {
                return Err(NtError::Message(format!(
                    "note id `{id}` already exists in index at {}",
                    existing.path.display()
                )));
            }
        }

        let active_ids: Vec<String> = self
            .notes
            .values()
            .filter(|note| note_is_in_vault_path(note, &active_vault))
            .map(|note| note.id.clone())
            .collect();

        for id in active_ids {
            self.remove_text_terms(&id);
        }

        self.notes
            .retain(|_, note| !note_is_in_vault_path(note, &active_vault));

        for (id, body) in bodies {
            self.refresh_text_terms(id, body);
        }

        self.notes.extend(notes);

        let existing_ids: BTreeSet<String> = self.notes.keys().cloned().collect();
        for note in self.notes.values_mut() {
            note.links.retain(|link| existing_ids.contains(link));
        }

        self.rebuild_derived();
        Ok(())
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

            for term in terms::terms_for_note(note) {
                self.terms.entry(term).or_default().push(note.id.clone());
            }
        }
    }
}

fn note_is_in_vault_path(note: &NoteMeta, vault_path: &Path) -> bool {
    validate_id(&note.id).is_ok() && note.path == vault_path.join(format!("{}.md", note.id))
}

impl Index {
    fn normalize_loaded(&mut self) {
        self.migrate_legacy_vaults();
        self.rebuild_derived();
    }

    fn migrate_legacy_vaults(&mut self) {
        let legacy_vaults = std::mem::take(&mut self.legacy_vaults);
        for (path, meta) in legacy_vaults {
            self.ensure_vault_for_path(PathBuf::from(path), meta.created);
        }

        if let Some(path) = self.legacy_active_notes_dir.take() {
            let name = self.ensure_vault_for_path(path, String::new());
            if self.active_vault.is_none() {
                self.active_vault = Some(name);
            }
        }
    }

    pub fn ensure_vault_for_path(&mut self, path: PathBuf, created: String) -> String {
        if let Some((name, _)) = self.vaults.iter().find(|(_, vault)| vault.path == path) {
            return name.clone();
        }

        let base = vault_name_from_path(&path);
        let name = unique_vault_name(&self.vaults, &base);
        self.vaults
            .insert(name.clone(), VaultMeta { path, created });
        name
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

fn vault_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("vault")
        .to_string()
}

fn unique_vault_name(vaults: &BTreeMap<String, VaultMeta>, base: &str) -> String {
    if !vaults.contains_key(base) {
        return base.to_string();
    }

    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if !vaults.contains_key(&candidate) {
            return candidate;
        }
    }

    unreachable!("unbounded suffix search should always find a vault name")
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
    fn normalizes_loaded_index_by_rebuilding_derived_maps() {
        let json = r#"{
            "version": 1,
            "active_vault": "notes",
            "vaults": {
                "notes": {
                    "path": "notes",
                    "created": "2026-05-28T14:30:12Z"
                }
            },
            "notes": {
                "NT20260528T143012": {
                    "id": "NT20260528T143012",
                    "path": "notes/NT20260528T143012.md",
                    "created": "2026-05-28T14:30:12Z",
                    "updated": "2026-05-28T14:30:12Z",
                    "title": "Storage",
                    "kind": "decision",
                    "status": "open",
                    "tags": ["design"],
                    "collections": ["projects/nt"],
                    "links": ["NT20260527T120000"],
                    "sources": []
                }
            },
            "recent": [],
            "tags": {}
        }"#;

        let mut index: Index = serde_json::from_str(json).unwrap();

        index.normalize_loaded();

        assert_eq!(index.recent, vec!["NT20260528T143012"]);
        assert_eq!(
            index.tags.get("design").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.collections.get("projects/nt").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
        assert_eq!(
            index.backlinks.get("NT20260527T120000").unwrap(),
            &vec!["NT20260528T143012".to_string()]
        );
    }

    #[test]
    fn removing_note_cleans_inbound_primary_links() {
        let mut index = Index::default();
        let mut first = note("NT20260528T143012");
        first.links = vec!["NT20260529T120000".to_string()];
        let second = note("NT20260529T120000");

        index.upsert_note(first);
        index.upsert_note(second);

        index.remove_notes(["NT20260529T120000"]);

        assert_eq!(index.notes["NT20260528T143012"].links, Vec::<String>::new());
        assert!(index.backlinks.is_empty());
    }

    #[test]
    fn indexes_body_and_heading_terms_without_full_body() {
        let mut index = Index::default();
        let id = "NT20260528T143012";

        index.upsert_note_with_body(
            note(id),
            "# Runtime Notes\n\nMicroVM jailer details and microvm follow-up.\n",
        );

        assert_eq!(index.body_terms["microvm"], vec![id.to_string()]);
        assert_eq!(index.heading_terms["runtime"], vec![id.to_string()]);
        assert_eq!(index.body_indexed, vec![id.to_string()]);
        assert!(index.body_terms_match(id, &["jailer".to_string()]).unwrap());

        let json = serde_json::to_string(&index).unwrap();
        assert!(!json.contains("MicroVM jailer details"));
    }

    #[test]
    fn refreshing_and_removing_note_updates_text_terms() {
        let mut index = Index::default();
        let id = "NT20260528T143012";

        index.upsert_note_with_body(note(id), "# Old\n\nalpha beta.\n");
        index.upsert_note_with_body(note(id), "# New\n\nbeta gamma.\n");

        assert!(!index.body_terms.contains_key("alpha"));
        assert_eq!(index.body_terms["gamma"], vec![id.to_string()]);
        assert!(!index.heading_terms.contains_key("old"));
        assert_eq!(index.heading_terms["new"], vec![id.to_string()]);

        index.remove_notes([id]);

        assert!(index.body_terms.is_empty());
        assert!(index.heading_terms.is_empty());
        assert!(index.body_indexed.is_empty());
    }
}
