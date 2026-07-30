use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{NtError, Result};
use crate::fs::database_path;
use crate::note::new_id;

const SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub vaults: BTreeMap<String, VaultMeta>,
    pub collections: BTreeMap<String, CollectionMeta>,
    pub notes: BTreeMap<String, NoteMeta>,
}

#[derive(Clone, Debug)]
pub struct VaultMeta {
    pub id: String,
    pub name: String,
    pub created: String,
}

#[derive(Clone, Debug)]
pub struct CollectionMeta {
    pub id: String,
    pub vault_id: String,
    pub name: String,
    pub created: String,
}

#[derive(Clone, Debug)]
pub struct NoteMeta {
    pub id: String,
    pub home_collection: String,
    pub body: String,
    pub created: String,
    pub updated: String,
    pub title: String,
    pub kind: String,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub due: Option<String>,
    pub closed: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub links: Vec<String>,
    pub sources: Vec<String>,
}

impl NoteMeta {
    pub fn new_note(
        id: String,
        home_collection: String,
        body: String,
        created: String,
        updated: String,
        title: String,
    ) -> Self {
        Self {
            id,
            home_collection: home_collection.clone(),
            body,
            created,
            updated,
            title,
            kind: "note".to_string(),
            status: None,
            priority: None,
            scheduled: None,
            due: None,
            closed: None,
            tags: Vec::new(),
            collections: vec![home_collection],
            links: Vec::new(),
            sources: Vec::new(),
        }
    }
}

impl Index {
    pub fn load() -> Result<Self> {
        let connection = open_database()?;
        let mut index = Self::default();

        {
            let mut statement =
                connection.prepare("SELECT id, name, created FROM vaults ORDER BY name")?;
            let rows = statement.query_map([], |row| {
                Ok(VaultMeta {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created: row.get(2)?,
                })
            })?;
            for row in rows {
                let vault = row?;
                index.vaults.insert(vault.name.clone(), vault);
            }
        }

        {
            let mut statement = connection.prepare(
                "SELECT c.id, c.vault_id, v.name, c.name, c.created
                 FROM collections c JOIN vaults v ON v.id = c.vault_id
                 ORDER BY v.name, c.name",
            )?;
            let rows = statement.query_map([], |row| {
                let vault_name: String = row.get(2)?;
                let name: String = row.get(3)?;
                Ok((
                    format!("{vault_name}/{name}"),
                    CollectionMeta {
                        id: row.get(0)?,
                        vault_id: row.get(1)?,
                        name,
                        created: row.get(4)?,
                    },
                ))
            })?;
            for row in rows {
                let (full_name, collection) = row?;
                index.collections.insert(full_name, collection);
            }
        }

        {
            let mut statement = connection.prepare(
                "SELECT n.id, v.name || '/' || c.name, n.body, n.created, n.updated,
                        n.title, n.kind, n.status, n.priority, n.scheduled, n.due, n.closed
                 FROM notes n
                 JOIN collections c ON c.id = n.home_collection_id
                 JOIN vaults v ON v.id = c.vault_id
                 ORDER BY n.created DESC, n.id DESC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(NoteMeta {
                    id: row.get(0)?,
                    home_collection: row.get(1)?,
                    body: row.get(2)?,
                    created: row.get(3)?,
                    updated: row.get(4)?,
                    title: row.get(5)?,
                    kind: row.get(6)?,
                    status: row.get(7)?,
                    priority: row.get(8)?,
                    scheduled: row.get(9)?,
                    due: row.get(10)?,
                    closed: row.get(11)?,
                    tags: Vec::new(),
                    collections: Vec::new(),
                    links: Vec::new(),
                    sources: Vec::new(),
                })
            })?;
            for row in rows {
                let note = row?;
                index.notes.insert(note.id.clone(), note);
            }
        }

        load_note_values(&connection, "note_tags", "tag", &mut index, |note| {
            &mut note.tags
        })?;
        load_note_values(&connection, "note_sources", "source", &mut index, |note| {
            &mut note.sources
        })?;
        load_note_values(&connection, "note_links", "target_id", &mut index, |note| {
            &mut note.links
        })?;

        let mut statement = connection.prepare(
            "SELECT nc.note_id, v.name || '/' || c.name
             FROM note_collections nc
             JOIN collections c ON c.id = nc.collection_id
             JOIN vaults v ON v.id = c.vault_id
             ORDER BY nc.note_id, v.name, c.name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (note_id, collection) = row?;
            if let Some(note) = index.notes.get_mut(&note_id) {
                note.collections.push(collection);
            }
        }

        Ok(index)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let mut connection = open_database()?;
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "PRAGMA defer_foreign_keys = ON;
             DELETE FROM note_tags;
             DELETE FROM note_sources;
             DELETE FROM note_links;
             DELETE FROM notes;
             DELETE FROM note_collections;
             DELETE FROM collections;
             DELETE FROM vaults;",
        )?;

        for vault in self.vaults.values() {
            transaction.execute(
                "INSERT INTO vaults (id, name, created) VALUES (?1, ?2, ?3)",
                params![vault.id, vault.name, vault.created],
            )?;
        }
        for collection in self.collections.values() {
            transaction.execute(
                "INSERT INTO collections (id, vault_id, name, created) VALUES (?1, ?2, ?3, ?4)",
                params![
                    collection.id,
                    collection.vault_id,
                    collection.name,
                    collection.created
                ],
            )?;
        }
        for note in self.notes.values() {
            let home_id = &self.collections[&note.home_collection].id;
            transaction.execute(
                "INSERT INTO notes
                 (id, home_collection_id, body, created, updated, title, kind, status,
                  priority, scheduled, due, closed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    note.id,
                    home_id,
                    note.body,
                    note.created,
                    note.updated,
                    note.title,
                    note.kind,
                    note.status,
                    note.priority,
                    note.scheduled,
                    note.due,
                    note.closed
                ],
            )?;
            for collection in &note.collections {
                transaction.execute(
                    "INSERT INTO note_collections (note_id, collection_id) VALUES (?1, ?2)",
                    params![note.id, self.collections[collection].id],
                )?;
            }
            insert_note_values(&transaction, "note_tags", "tag", &note.id, &note.tags)?;
            insert_note_values(
                &transaction,
                "note_sources",
                "source",
                &note.id,
                &note.sources,
            )?;
            insert_note_values(
                &transaction,
                "note_links",
                "target_id",
                &note.id,
                &note.links,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn all_notes(&self) -> Vec<&NoteMeta> {
        let mut notes: Vec<_> = self.notes.values().collect();
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
        for id in &ids {
            self.notes.remove(*id);
        }
        for note in self.notes.values_mut() {
            note.links.retain(|link| !ids.contains(link.as_str()));
        }
    }

    pub fn create_vault(&mut self, name: &str, created: &str) -> Result<()> {
        validate_namespace_part(name, "vault")?;
        if self.vaults.contains_key(name) {
            return Err(NtError::Message(format!("vault `{name}` already exists")));
        }
        self.vaults.insert(
            name.to_string(),
            VaultMeta {
                id: new_id(),
                name: name.to_string(),
                created: created.to_string(),
            },
        );
        self.ensure_collection(&format!("{name}/inbox"), created)?;
        Ok(())
    }

    pub fn ensure_collection(&mut self, full_name: &str, created: &str) -> Result<()> {
        if self.collections.contains_key(full_name) {
            return Ok(());
        }
        let (vault_name, collection_name) = parse_collection_name(full_name)?;
        let vault = self.vaults.get(vault_name).ok_or_else(|| {
            NtError::Message(format!(
                "unknown vault `{vault_name}`; run `nt init {vault_name}` first"
            ))
        })?;
        self.collections.insert(
            full_name.to_string(),
            CollectionMeta {
                id: new_id(),
                vault_id: vault.id.clone(),
                name: collection_name.to_string(),
                created: created.to_string(),
            },
        );
        Ok(())
    }

    pub fn default_home_collection(&self) -> Result<String> {
        if self.vaults.len() != 1 {
            return Err(NtError::Message(
                "specify `home:<vault>/<collection>` when more than one vault exists".to_string(),
            ));
        }
        let vault = self.vaults.keys().next().ok_or(NtError::MissingVault)?;
        Ok(format!("{vault}/inbox"))
    }

    fn validate(&self) -> Result<()> {
        for note in self.notes.values() {
            if !self.collections.contains_key(&note.home_collection) {
                return Err(NtError::Message(format!(
                    "unknown home collection `{}` for note {}",
                    note.home_collection, note.id
                )));
            }
            if !note.collections.contains(&note.home_collection) {
                return Err(NtError::Message(format!(
                    "home collection `{}` is not a membership of note {}",
                    note.home_collection, note.id
                )));
            }
            for collection in &note.collections {
                if !self.collections.contains_key(collection) {
                    return Err(NtError::Message(format!(
                        "unknown collection `{collection}` for note {}",
                        note.id
                    )));
                }
            }
        }
        Ok(())
    }
}

pub fn parse_collection_name(value: &str) -> Result<(&str, &str)> {
    let Some((vault, collection)) = value.split_once('/') else {
        return Err(NtError::Message(format!(
            "invalid collection `{value}`; use <vault>/<collection>"
        )));
    };
    validate_namespace_part(vault, "vault")?;
    validate_namespace_part(collection, "collection")?;
    Ok((vault, collection))
}

fn validate_namespace_part(value: &str, kind: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_uppercase() || ch == ',')
    {
        return Err(NtError::Message(format!(
            "invalid {kind} `{value}`; use lowercase names without spaces or commas"
        )));
    }
    Ok(())
}

fn open_database() -> Result<Connection> {
    let path = database_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = DELETE;
         CREATE TABLE IF NOT EXISTS schema_version (
             version INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS vaults (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL UNIQUE,
             created TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS collections (
             id TEXT PRIMARY KEY,
             vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
             name TEXT NOT NULL,
             created TEXT NOT NULL,
             UNIQUE(vault_id, name)
         );
         CREATE TABLE IF NOT EXISTS notes (
             id TEXT PRIMARY KEY,
             home_collection_id TEXT NOT NULL,
             body TEXT NOT NULL,
             created TEXT NOT NULL,
             updated TEXT NOT NULL,
             title TEXT NOT NULL,
             kind TEXT NOT NULL CHECK(kind IN ('note', 'todo')),
             status TEXT,
             priority TEXT,
             scheduled TEXT,
             due TEXT,
             closed TEXT,
             UNIQUE(id, home_collection_id),
             FOREIGN KEY(id, home_collection_id)
                 REFERENCES note_collections(note_id, collection_id)
                 DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TABLE IF NOT EXISTS note_collections (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE RESTRICT,
             PRIMARY KEY(note_id, collection_id)
         );
         CREATE TABLE IF NOT EXISTS note_tags (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             tag TEXT NOT NULL,
             PRIMARY KEY(note_id, tag)
         );
         CREATE TABLE IF NOT EXISTS note_sources (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             source TEXT NOT NULL,
             PRIMARY KEY(note_id, source)
         );
         CREATE TABLE IF NOT EXISTS note_links (
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             PRIMARY KEY(note_id, target_id)
         );
         CREATE INDEX IF NOT EXISTS notes_created ON notes(created DESC, id DESC);
         CREATE INDEX IF NOT EXISTS note_collections_collection ON note_collections(collection_id);
         CREATE INDEX IF NOT EXISTS note_tags_tag ON note_tags(tag);
         CREATE INDEX IF NOT EXISTS note_links_target ON note_links(target_id);",
    )?;
    let version: Option<i64> = connection
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    match version {
        None => {
            connection.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
        }
        Some(SCHEMA_VERSION) => {}
        Some(version) => {
            return Err(NtError::Message(format!(
                "unsupported database schema version {version}"
            )));
        }
    }
    Ok(connection)
}

fn load_note_values(
    connection: &Connection,
    table: &str,
    column: &str,
    index: &mut Index,
    values: impl Fn(&mut NoteMeta) -> &mut Vec<String>,
) -> Result<()> {
    let sql = format!("SELECT note_id, {column} FROM {table} ORDER BY note_id, {column}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (note_id, value) = row?;
        if let Some(note) = index.notes.get_mut(&note_id) {
            values(note).push(value);
        }
    }
    Ok(())
}

fn insert_note_values(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    note_id: &str,
    values: &[String],
) -> Result<()> {
    let sql = format!("INSERT INTO {table} (note_id, {column}) VALUES (?1, ?2)");
    for value in values {
        transaction.execute(&sql, params![note_id, value])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_collection_name;

    #[test]
    fn collection_names_are_vault_qualified() {
        assert_eq!(
            parse_collection_name("personal/rust").unwrap(),
            ("personal", "rust")
        );
        assert!(parse_collection_name("rust").is_err());
        assert!(parse_collection_name("Personal/rust").is_err());
    }
}
