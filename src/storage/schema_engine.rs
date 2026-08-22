use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::error::{NtError, Result};

#[derive(Clone, Copy)]
pub(crate) struct SchemaObject {
    pub(crate) object_type: &'static str,
    pub(crate) name: &'static str,
    pub(crate) sql: &'static str,
}

pub(crate) struct SchemaManifest {
    pub(crate) application_id: i64,
    pub(crate) version: i64,
    pub(crate) objects: &'static [SchemaObject],
    pub(crate) required_shadow_tables: &'static [&'static str],
    pub(crate) allowed_triggers: &'static [&'static str],
    pub(crate) version_insert_sql: &'static str,
}

impl SchemaManifest {
    pub(crate) fn step_count(&self) -> usize {
        1 + self.objects.len()
    }

    #[cfg(test)]
    pub(crate) fn steps(&self) -> Vec<&'static str> {
        self.objects
            .iter()
            .map(|object| object.sql)
            .chain(std::iter::once(self.version_insert_sql))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Identity {
    Empty,
    Nt,
}

pub(crate) fn inspect(connection: &Connection, manifest: &SchemaManifest) -> Result<Identity> {
    let application_id: i64 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let object_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;

    if application_id == 0 {
        return if object_count == 0 {
            Ok(Identity::Empty)
        } else {
            Err(NtError::NotNtDatabase)
        };
    }
    if application_id != manifest.application_id {
        return Err(NtError::NotNtDatabase);
    }

    validate_version(connection, manifest)?;
    validate_required_schema(connection, manifest)?;
    Ok(Identity::Nt)
}

pub(crate) fn initialize(connection: &mut Connection, manifest: &SchemaManifest) -> Result<bool> {
    if inspect(connection, manifest)? == Identity::Nt {
        return Ok(false);
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match inspect(&transaction, manifest)? {
        Identity::Nt => {
            transaction.commit()?;
            Ok(false)
        }
        Identity::Empty => {
            initialize_transaction(&transaction, manifest, |_| Ok(()))?;
            transaction.commit()?;
            Ok(true)
        }
    }
}

#[cfg(test)]
pub(crate) fn initialize_with(
    connection: &mut Connection,
    manifest: &SchemaManifest,
    mut after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    initialize_transaction(&transaction, manifest, &mut after_step)?;
    transaction.commit()?;
    Ok(())
}

fn initialize_transaction(
    transaction: &rusqlite::Transaction<'_>,
    manifest: &SchemaManifest,
    mut after_step: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut step = 0;
    for object in manifest.objects {
        transaction.execute_batch(object.sql)?;
        after_step(step)?;
        step += 1;
    }
    transaction.execute_batch(manifest.version_insert_sql)?;
    after_step(step)?;
    step += 1;
    debug_assert_eq!(step, manifest.step_count());
    transaction.pragma_update(None, "application_id", manifest.application_id)?;
    after_step(step)?;
    validate_version(transaction, manifest)?;
    validate_required_schema(transaction, manifest)?;
    Ok(())
}

fn validate_version(connection: &Connection, manifest: &SchemaManifest) -> Result<()> {
    let has_table = connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_table {
        return Err(NtError::NotNtDatabase);
    }
    let has_version_column: bool = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM pragma_table_info('schema_version') WHERE name = 'version'
         )",
        [],
        |row| row.get(0),
    )?;
    if !has_version_column {
        return Err(NtError::NotNtDatabase);
    }

    let mut statement = connection.prepare("SELECT version FROM schema_version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, Value>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match versions.as_slice() {
        [Value::Integer(version)] if *version == manifest.version => Ok(()),
        [Value::Integer(version)] => Err(NtError::UnsupportedSchema(*version)),
        _ => Err(NtError::NotNtDatabase),
    }
}

fn validate_required_schema(connection: &Connection, manifest: &SchemaManifest) -> Result<()> {
    for object in manifest.objects {
        validate_object(connection, *object)?;
    }

    let singleton: Option<i64> = connection
        .query_row(
            "SELECT singleton FROM schema_version WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if singleton != Some(1) {
        return Err(NtError::NotNtDatabase);
    }
    for name in manifest.required_shadow_tables {
        let exists = connection
            .query_row(
                "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(NtError::NotNtDatabase);
        }
    }

    let mut statement =
        connection.prepare("SELECT name FROM sqlite_schema WHERE type = 'trigger'")?;
    let triggers = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if triggers
        .iter()
        .any(|name| !manifest.allowed_triggers.contains(&name.as_str()))
    {
        return Err(NtError::NotNtDatabase);
    }
    Ok(())
}

fn validate_object(connection: &Connection, object: SchemaObject) -> Result<()> {
    let stored_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
            (object.object_type, object.name),
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if stored_sql.as_deref() != Some(object.sql) {
        return Err(NtError::NotNtDatabase);
    }
    Ok(())
}
