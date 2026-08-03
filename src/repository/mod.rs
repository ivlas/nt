use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::error::{NtError, Result};
use crate::fs::database_path;
use crate::note::new_id;

const SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug)]
pub struct VaultMeta {
    pub id: String,
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

#[derive(Debug)]
pub enum NoteChange {
    Kind(String),
    Status(Option<String>),
    Priority(Option<String>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Home(String),
    Tag { add: bool, value: String },
    Collection { add: bool, value: String },
    Link { add: bool, value: String },
    Source { add: bool, value: String },
}

pub struct Repository {
    connection: Connection,
}

impl Repository {
    pub fn open() -> Result<Self> {
        let path = database_path()?;
        Self::open_path(&path)
    }

    fn open_path(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        configure_and_initialize(&connection)?;
        Ok(Self { connection })
    }

    pub fn create_vault(&mut self, name: &str, created: &str) -> Result<VaultMeta> {
        validate_namespace_part(name, "vault")?;
        let transaction = self.connection.transaction()?;
        let exists = transaction
            .query_row("SELECT 1 FROM vaults WHERE name = ?1", [name], |_| Ok(()))
            .optional()?
            .is_some();
        if exists {
            return Err(NtError::Message(format!("vault `{name}` already exists")));
        }

        let vault = VaultMeta {
            id: new_id(),
            name: name.to_string(),
            created: created.to_string(),
        };
        transaction.execute(
            "INSERT INTO vaults (id, name, created) VALUES (?1, ?2, ?3)",
            params![vault.id, vault.name, vault.created],
        )?;
        transaction.execute(
            "INSERT INTO collections (id, vault_id, name, created) VALUES (?1, ?2, 'inbox', ?3)",
            params![new_id(), vault.id, created],
        )?;
        transaction.commit()?;
        Ok(vault)
    }

    pub fn list_vaults(&self) -> Result<Vec<VaultMeta>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, created FROM vaults ORDER BY name")?;
        let rows = statement.query_map([], |row| {
            Ok(VaultMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                created: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_collections(&self) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT v.name || '/' || c.name
             FROM collections c JOIN vaults v ON v.id = c.vault_id
             ORDER BY v.name, c.name",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn default_home_collection(&self) -> Result<String> {
        let mut statement = self
            .connection
            .prepare("SELECT name FROM vaults ORDER BY name")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match names.as_slice() {
            [] => Err(NtError::MissingVault),
            [name] => Ok(format!("{name}/inbox")),
            _ => Err(NtError::Message(
                "specify `home:<vault>/<collection>` when more than one vault exists".to_string(),
            )),
        }
    }

    pub fn note_exists(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row("SELECT 1 FROM notes WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    pub fn insert_note(&mut self, note: &NoteMeta) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute_batch("PRAGMA defer_foreign_keys = ON")?;

        let mut collection_ids = Vec::new();
        for collection in &note.collections {
            let id = ensure_collection(&transaction, collection, &note.created)?;
            collection_ids.push(id);
        }
        let home_id = ensure_collection(&transaction, &note.home_collection, &note.created)?;
        for link in &note.links {
            if !note_exists(&transaction, link)? {
                return Err(NtError::NoteNotFound(link.clone()));
            }
        }

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

        let mut memberships: BTreeSet<String> = collection_ids.into_iter().collect();
        memberships.insert(home_id);
        for collection_id in memberships {
            transaction.execute(
                "INSERT INTO note_collections (note_id, collection_id) VALUES (?1, ?2)",
                params![note.id, collection_id],
            )?;
        }
        insert_values(&transaction, "note_tags", "tag", &note.id, &note.tags)?;
        insert_values(
            &transaction,
            "note_sources",
            "source",
            &note.id,
            &note.sources,
        )?;
        insert_values(
            &transaction,
            "note_links",
            "target_id",
            &note.id,
            &note.links,
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_note(&self, id: &str) -> Result<NoteMeta> {
        let transaction = self.connection.unchecked_transaction()?;
        let note = load_note(&transaction, id)?;
        transaction.commit()?;
        Ok(note)
    }

    pub fn list_notes(&self) -> Result<Vec<NoteMeta>> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut statement = transaction.prepare(
            "SELECT n.id, v.name || '/' || c.name, n.body, n.created, n.updated,
                    n.title, n.kind, n.status, n.priority, n.scheduled, n.due, n.closed
             FROM notes n
             JOIN collections c ON c.id = n.home_collection_id
             JOIN vaults v ON v.id = c.vault_id
             ORDER BY n.created DESC, n.id DESC",
        )?;
        let rows = statement.query_map([], note_from_row)?;
        let mut notes = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for note in &mut notes {
            load_relationships(&transaction, note)?;
        }
        transaction.commit()?;
        Ok(notes)
    }

    pub fn update_note(&mut self, id: &str, change: &NoteChange, now: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        let (kind, status, closed, home) = transaction
            .query_row(
                "SELECT n.kind, n.status, n.closed, v.name || '/' || c.name
                 FROM notes n
                 JOIN collections c ON c.id = n.home_collection_id
                 JOIN vaults v ON v.id = c.vault_id
                 WHERE n.id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;

        match change {
            NoteChange::Kind(value) => {
                if value == "note" {
                    transaction.execute(
                        "UPDATE notes SET kind = ?1, status = NULL, priority = NULL,
                         scheduled = NULL, due = NULL, closed = NULL WHERE id = ?2",
                        params![value, id],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE notes SET kind = ?1 WHERE id = ?2",
                        params![value, id],
                    )?;
                }
            }
            NoteChange::Status(value) => {
                ensure_todo_field(&kind, value.is_some(), "status")?;
                let next_closed = if value.as_deref().is_some_and(is_terminal_status) {
                    if status == *value {
                        closed
                    } else {
                        Some(now.to_string())
                    }
                } else {
                    None
                };
                transaction.execute(
                    "UPDATE notes SET status = ?1, closed = ?2 WHERE id = ?3",
                    params![value, next_closed, id],
                )?;
            }
            NoteChange::Priority(value) => {
                ensure_todo_field(&kind, value.is_some(), "priority")?;
                transaction.execute(
                    "UPDATE notes SET priority = ?1 WHERE id = ?2",
                    params![value, id],
                )?;
            }
            NoteChange::Scheduled(value) => {
                ensure_todo_field(&kind, value.is_some(), "scheduled")?;
                transaction.execute(
                    "UPDATE notes SET scheduled = ?1 WHERE id = ?2",
                    params![value, id],
                )?;
            }
            NoteChange::Due(value) => {
                ensure_todo_field(&kind, value.is_some(), "due")?;
                transaction.execute(
                    "UPDATE notes SET due = ?1 WHERE id = ?2",
                    params![value, id],
                )?;
            }
            NoteChange::Home(collection) => {
                let collection_id = ensure_collection(&transaction, collection, now)?;
                transaction.execute(
                    "INSERT INTO note_collections (note_id, collection_id) VALUES (?1, ?2)
                     ON CONFLICT DO NOTHING",
                    params![id, collection_id],
                )?;
                transaction.execute(
                    "UPDATE notes SET home_collection_id = ?1 WHERE id = ?2",
                    params![collection_id, id],
                )?;
            }
            NoteChange::Collection { add, value } => {
                if *add {
                    let collection_id = ensure_collection(&transaction, value, now)?;
                    transaction.execute(
                        "INSERT INTO note_collections (note_id, collection_id) VALUES (?1, ?2)
                         ON CONFLICT DO NOTHING",
                        params![id, collection_id],
                    )?;
                } else {
                    if home == *value {
                        return Err(NtError::Message(format!(
                            "cannot remove home collection `{value}`; move home first"
                        )));
                    }
                    if let Some(collection_id) = collection_id(&transaction, value)? {
                        transaction.execute(
                            "DELETE FROM note_collections WHERE note_id = ?1 AND collection_id = ?2",
                            params![id, collection_id],
                        )?;
                    }
                }
            }
            NoteChange::Tag { add, value } => {
                change_value(&transaction, "note_tags", "tag", id, value, *add)?;
            }
            NoteChange::Source { add, value } => {
                change_value(&transaction, "note_sources", "source", id, value, *add)?;
            }
            NoteChange::Link { add, value } => {
                if *add && !note_exists(&transaction, value)? {
                    return Err(NtError::NoteNotFound(value.clone()));
                }
                change_value(&transaction, "note_links", "target_id", id, value, *add)?;
            }
        }

        transaction.execute(
            "UPDATE notes SET updated = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_edited_note(
        &mut self,
        id: &str,
        expected_updated: &str,
        expected_body: &str,
        body: &str,
        title: &str,
        updated: &str,
    ) -> Result<()> {
        let body_sources = crate::note::sources_from_body(body);
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE notes SET body = ?1, title = ?2, updated = ?3
             WHERE id = ?4 AND updated = ?5 AND body = ?6",
            params![body, title, updated, id, expected_updated, expected_body],
        )?;
        if changed == 0 {
            return Err(NtError::Message(
                "note changed during edit; please retry".to_string(),
            ));
        }
        for source in &body_sources {
            transaction.execute(
                "INSERT INTO note_sources (note_id, source) VALUES (?1, ?2)
                 ON CONFLICT DO NOTHING",
                params![id, source],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_notes(&mut self, ids: &[String]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for id in ids {
            if !note_exists(&transaction, id)? {
                return Err(NtError::NoteNotFound(id.clone()));
            }
        }
        for id in ids {
            transaction.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        }
        transaction.commit()?;
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

fn configure_and_initialize(connection: &Connection) -> Result<()> {
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
    Ok(())
}

fn ensure_collection(
    transaction: &Transaction<'_>,
    full_name: &str,
    created: &str,
) -> Result<String> {
    if let Some(id) = collection_id(transaction, full_name)? {
        return Ok(id);
    }
    let (vault_name, collection_name) = parse_collection_name(full_name)?;
    let vault_id = transaction
        .query_row(
            "SELECT id FROM vaults WHERE name = ?1",
            [vault_name],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            NtError::Message(format!(
                "unknown vault `{vault_name}`; run `nt init {vault_name}` first"
            ))
        })?;
    let id = new_id();
    transaction.execute(
        "INSERT INTO collections (id, vault_id, name, created) VALUES (?1, ?2, ?3, ?4)",
        params![id, vault_id, collection_name, created],
    )?;
    Ok(id)
}

fn collection_id(connection: &Connection, full_name: &str) -> Result<Option<String>> {
    let (vault, collection) = parse_collection_name(full_name)?;
    connection
        .query_row(
            "SELECT c.id FROM collections c JOIN vaults v ON v.id = c.vault_id
             WHERE v.name = ?1 AND c.name = ?2",
            params![vault, collection],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NoteMeta> {
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
}

fn load_note(connection: &Connection, id: &str) -> Result<NoteMeta> {
    let mut note = connection
        .query_row(
            "SELECT n.id, v.name || '/' || c.name, n.body, n.created, n.updated,
                    n.title, n.kind, n.status, n.priority, n.scheduled, n.due, n.closed
             FROM notes n
             JOIN collections c ON c.id = n.home_collection_id
             JOIN vaults v ON v.id = c.vault_id
             WHERE n.id = ?1",
            [id],
            note_from_row,
        )
        .optional()?
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))?;
    load_relationships(connection, &mut note)?;
    Ok(note)
}

fn load_relationships(connection: &Connection, note: &mut NoteMeta) -> Result<()> {
    note.tags = load_values(connection, "note_tags", "tag", &note.id)?;
    note.sources = load_values(connection, "note_sources", "source", &note.id)?;
    note.links = load_values(connection, "note_links", "target_id", &note.id)?;
    let mut statement = connection.prepare(
        "SELECT v.name || '/' || c.name
         FROM note_collections nc
         JOIN collections c ON c.id = nc.collection_id
         JOIN vaults v ON v.id = c.vault_id
         WHERE nc.note_id = ?1
         ORDER BY v.name, c.name",
    )?;
    note.collections = statement
        .query_map([&note.id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(())
}

fn load_values(
    connection: &Connection,
    table: &str,
    column: &str,
    id: &str,
) -> Result<Vec<String>> {
    let sql = format!("SELECT {column} FROM {table} WHERE note_id = ?1 ORDER BY {column}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn insert_values(
    transaction: &Transaction<'_>,
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

fn change_value(
    transaction: &Transaction<'_>,
    table: &str,
    column: &str,
    note_id: &str,
    value: &str,
    add: bool,
) -> Result<()> {
    let action = if add {
        format!("INSERT INTO {table} (note_id, {column}) VALUES (?1, ?2) ON CONFLICT DO NOTHING")
    } else {
        format!("DELETE FROM {table} WHERE note_id = ?1 AND {column} = ?2")
    };
    transaction.execute(&action, params![note_id, value])?;
    Ok(())
}

fn note_exists(connection: &Connection, id: &str) -> Result<bool> {
    Ok(connection
        .query_row("SELECT 1 FROM notes WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

fn ensure_todo_field(kind: &str, has_value: bool, field: &str) -> Result<()> {
    if has_value && kind != "todo" {
        Err(NtError::Message(format!(
            "`{field}` metadata is only valid for todo notes"
        )))
    } else {
        Ok(())
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "done" | "dropped")
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
