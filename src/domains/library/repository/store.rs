use std::str::FromStr;

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter};

use super::super::{
    LibraryCapture, LibraryHistoryRow, LibraryItem, LibraryItemId, LibraryQuery, LibrarySummary,
    LibrarySummaryRow, LibraryTimestamp, NewLibraryCapture, NewLibraryItem, timestamp_now,
};
use super::query_sql::compile_query;
use super::{CreateItemOutcome, Repository};
use crate::error::{NtError, Result, StoredLibraryContext};

impl Repository {
    pub fn create_item(&mut self, item: NewLibraryItem) -> Result<CreateItemOutcome> {
        let hash = content_hash(item.capture().content());
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((pk, id)) = item_identity_by_source(&transaction, item.source().as_str())? {
            let capture_created = insert_capture_if_new(&transaction, pk, item.capture(), &hash)?;
            transaction.commit()?;
            return Ok(CreateItemOutcome {
                id,
                item_created: false,
                capture_created,
            });
        }

        let id = LibraryItemId::generate()?;
        let now = timestamp_now()?;
        transaction.execute(
            "INSERT INTO library_items(id, source, title, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![
                id.to_string(),
                item.source().as_str(),
                item.title(),
                now.as_str()
            ],
        )?;
        let item_pk = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO library_captures(item_pk, captured, content, content_hash)
             VALUES (?1, ?2, ?3, ?4)",
            params![item_pk, now.as_str(), item.capture().content(), hash],
        )?;
        transaction.commit()?;
        Ok(CreateItemOutcome {
            id,
            item_created: true,
            capture_created: true,
        })
    }

    pub fn capture(&mut self, id: &LibraryItemId, capture: NewLibraryCapture) -> Result<bool> {
        let hash = content_hash(capture.content());
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item_pk = item_pk(&transaction, id)?;
        let inserted = insert_capture_if_new(&transaction, item_pk, &capture, &hash)?;
        transaction.commit()?;
        Ok(inserted)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn update_title(&mut self, id: &LibraryItemId, title: &str) -> Result<bool> {
        if title.trim().is_empty() {
            return Err(NtError::InvalidValue {
                field: "library title",
                value: title.to_string(),
            });
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        item_pk(&transaction, id)?;
        let now = timestamp_now()?;
        let changed = transaction.execute(
            "UPDATE library_items SET title = ?1, updated = ?2
             WHERE id = ?3 AND title <> ?1",
            params![title, now.as_str(), id.to_string()],
        )? != 0;
        transaction.commit()?;
        Ok(changed)
    }

    pub fn replace_latest_summary(
        &mut self,
        id: &LibraryItemId,
        summary: &str,
        generator: &str,
        version: &str,
    ) -> Result<()> {
        for (field, value) in [
            ("library summary", summary),
            ("summary generator", generator),
            ("summary version", version),
        ] {
            if value.trim().is_empty() {
                return Err(NtError::InvalidValue {
                    field,
                    value: value.to_string(),
                });
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item_pk = item_pk(&transaction, id)?;
        let capture_pk = latest_capture_pk(&transaction, item_pk)?;
        let now = timestamp_now()?;
        transaction.execute(
            "INSERT INTO library_summaries(capture_pk, summary, generator, version, created)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(capture_pk) DO UPDATE SET
                 summary = excluded.summary,
                 generator = excluded.generator,
                 version = excluded.version,
                 created = excluded.created",
            params![capture_pk, summary, generator, version, now.as_str()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_item(&self, id: &LibraryItemId) -> Result<LibraryItem> {
        load_item(&self.connection, id)
    }

    pub fn get_latest_capture(&self, id: &LibraryItemId) -> Result<LibraryCapture> {
        let item_pk = item_pk_on_connection(&self.connection, id)?;
        load_capture(
            &self.connection,
            "SELECT pk, captured, content, content_hash
             FROM library_captures WHERE item_pk = ?1
             ORDER BY captured DESC, pk DESC LIMIT 1",
            item_pk,
            id,
        )
    }

    pub fn history(&self, id: &LibraryItemId) -> Result<Vec<LibraryHistoryRow>> {
        let item_pk = item_pk_on_connection(&self.connection, id)?;
        let mut statement = self.connection.prepare(
            "SELECT c.pk, c.captured, c.content, c.content_hash,
                    s.summary, s.generator, s.version, s.created
             FROM library_captures c
             LEFT JOIN library_summaries s ON s.capture_pk = c.pk
             WHERE c.item_pk = ?1
             ORDER BY c.captured, c.pk",
        )?;
        let mut rows = statement.query([item_pk])?;
        let mut history = Vec::new();
        while let Some(row) = rows.next()? {
            let context = StoredLibraryContext::new(Some(id.to_string()), row.get(0)?);
            let capture = LibraryCapture::rehydrate(
                row.get(0)?,
                decode_timestamp(row.get(1)?, &context, "captured")?,
                row.get(2)?,
                row.get(3)?,
            )
            .map_err(|error| stored_error(context.clone(), "capture", error))?;
            let summary = match row.get::<_, Option<String>>(4)? {
                Some(summary) => Some(
                    LibrarySummary::rehydrate(
                        summary,
                        row.get(5)?,
                        row.get(6)?,
                        decode_timestamp(row.get(7)?, &context, "summary created")?,
                    )
                    .map_err(|error| stored_error(context.clone(), "summary", error))?,
                ),
                None => None,
            };
            history.push(LibraryHistoryRow::new(capture, summary));
        }
        Ok(history)
    }

    pub fn visit_summaries(
        &self,
        query: &LibraryQuery,
        mut visit: impl FnMut(LibrarySummaryRow) -> Result<()>,
    ) -> Result<()> {
        let (where_sql, mut parameters) = compile_query(query);
        let limit_sql = if let Some(limit) = query.limit() {
            parameters.push(Value::Integer(limit));
            format!("LIMIT ?{}", parameters.len())
        } else {
            String::new()
        };
        let sql = format!(
            "SELECT i.pk, i.id, i.source, i.title, i.created, i.updated,
                    c.captured, s.summary
             FROM library_items i
             JOIN library_captures c ON c.pk = (
                 SELECT latest.pk FROM library_captures latest
                 WHERE latest.item_pk = i.pk
                 ORDER BY latest.captured DESC, latest.pk DESC LIMIT 1
             )
             LEFT JOIN library_summaries s ON s.capture_pk = c.pk
             {where_sql}
             ORDER BY i.updated DESC, i.id DESC
             {limit_sql}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(parameters))?;
        while let Some(row) = rows.next()? {
            let stored_id: String = row.get(1)?;
            let context = StoredLibraryContext::new(Some(stored_id.clone()), row.get(0)?);
            let item = rehydrate_item(
                &context,
                stored_id,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            )?;
            visit(LibrarySummaryRow::new(
                item,
                decode_timestamp(row.get(6)?, &context, "captured")?,
                row.get(7)?,
            ))?;
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn delete_item(&mut self, id: &LibraryItemId) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pk = item_pk(&transaction, id)?;
        transaction.execute("DELETE FROM library_items WHERE pk = ?1", [pk])?;
        transaction.commit()?;
        Ok(())
    }
}

fn insert_capture_if_new(
    transaction: &Transaction<'_>,
    item_pk: i64,
    capture: &NewLibraryCapture,
    hash: &str,
) -> Result<bool> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM library_captures WHERE item_pk = ?1 AND content_hash = ?2
         )",
        params![item_pk, hash],
        |row| row.get(0),
    )?;
    if exists {
        return Ok(false);
    }
    let now = timestamp_now()?;
    transaction.execute(
        "INSERT INTO library_captures(item_pk, captured, content, content_hash)
         VALUES (?1, ?2, ?3, ?4)",
        params![item_pk, now.as_str(), capture.content(), hash],
    )?;
    transaction.execute(
        "UPDATE library_items SET updated = ?1 WHERE pk = ?2",
        params![now.as_str(), item_pk],
    )?;
    Ok(true)
}

fn content_hash(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

fn item_identity_by_source(
    transaction: &Transaction<'_>,
    source: &str,
) -> Result<Option<(i64, LibraryItemId)>> {
    let stored = transaction
        .query_row(
            "SELECT pk, id FROM library_items WHERE source = ?1",
            [source],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    stored
        .map(|(pk, id)| {
            id.parse().map(|id| (pk, id)).map_err(|error| {
                stored_error(StoredLibraryContext::new(Some(id), Some(pk)), "id", error)
            })
        })
        .transpose()
}

pub(crate) fn item_pk(transaction: &Transaction<'_>, id: &LibraryItemId) -> Result<i64> {
    transaction
        .query_row(
            "SELECT pk FROM library_items WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| NtError::LibraryItemNotFound(id.to_string()))
}

fn item_pk_on_connection(connection: &rusqlite::Connection, id: &LibraryItemId) -> Result<i64> {
    connection
        .query_row(
            "SELECT pk FROM library_items WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| NtError::LibraryItemNotFound(id.to_string()))
}

fn latest_capture_pk(transaction: &Transaction<'_>, item_pk: i64) -> Result<i64> {
    transaction
        .query_row(
            "SELECT pk FROM library_captures WHERE item_pk = ?1
         ORDER BY captured DESC, pk DESC LIMIT 1",
            [item_pk],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_item(connection: &rusqlite::Connection, id: &LibraryItemId) -> Result<LibraryItem> {
    let stored = connection
        .query_row(
            "SELECT pk, id, source, title, created, updated FROM library_items WHERE id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| NtError::LibraryItemNotFound(id.to_string()))?;
    let context = StoredLibraryContext::new(Some(stored.1.clone()), Some(stored.0));
    rehydrate_item(&context, stored.1, stored.2, stored.3, stored.4, stored.5)
}

fn rehydrate_item(
    context: &StoredLibraryContext,
    id: String,
    source: String,
    title: String,
    created: String,
    updated: String,
) -> Result<LibraryItem> {
    LibraryItem::rehydrate(
        LibraryItemId::from_str(&id).map_err(|error| stored_error(context.clone(), "id", error))?,
        source,
        title,
        decode_timestamp(created, context, "created")?,
        decode_timestamp(updated, context, "updated")?,
    )
    .map_err(|error| stored_error(context.clone(), "item", error))
}

fn load_capture(
    connection: &rusqlite::Connection,
    sql: &str,
    item_pk: i64,
    id: &LibraryItemId,
) -> Result<LibraryCapture> {
    let stored = connection.query_row(sql, [item_pk], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let context = StoredLibraryContext::new(Some(id.to_string()), Some(stored.0));
    LibraryCapture::rehydrate(
        stored.0,
        decode_timestamp(stored.1, &context, "captured")?,
        stored.2,
        stored.3,
    )
    .map_err(|error| stored_error(context, "capture", error))
}

fn decode_timestamp(
    value: String,
    context: &StoredLibraryContext,
    field: &'static str,
) -> Result<LibraryTimestamp> {
    value
        .parse()
        .map_err(|error| stored_error(context.clone(), field, error))
}

fn stored_error(context: StoredLibraryContext, field: &'static str, error: NtError) -> NtError {
    NtError::invalid_stored_library_with_source(context, field, error)
}

#[cfg(test)]
pub(super) fn hash_for_test(content: &str) -> String {
    content_hash(content)
}
