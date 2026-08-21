use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::domains::library::LibraryItemId;
use crate::domains::note::NoteId;
use crate::error::{NtError, Result};

pub struct NoteLibraryRepository {
    pub(crate) connection: rusqlite::Connection,
}

impl NoteLibraryRepository {
    pub(crate) fn from_connection(connection: rusqlite::Connection) -> Self {
        Self { connection }
    }

    pub fn reference(&mut self, note_id: &NoteId, library_id: &LibraryItemId) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let note_pk = note_pk(&transaction, note_id)?;
        let library_pk = library_pk(&transaction, library_id)?;
        let changed = transaction.execute(
            "INSERT OR IGNORE INTO note_library_refs(note_pk, library_item_pk)
             VALUES (?1, ?2)",
            params![note_pk, library_pk],
        )? != 0;
        transaction.commit()?;
        Ok(changed)
    }

    pub fn unreference(&mut self, note_id: &NoteId, library_id: &LibraryItemId) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let note_pk = note_pk(&transaction, note_id)?;
        let library_pk = library_pk(&transaction, library_id)?;
        let changed = transaction.execute(
            "DELETE FROM note_library_refs WHERE note_pk = ?1 AND library_item_pk = ?2",
            params![note_pk, library_pk],
        )? != 0;
        transaction.commit()?;
        Ok(changed)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn libraries_for_note(&self, note_id: &NoteId) -> Result<Vec<LibraryItemId>> {
        ensure_note_exists(&self.connection, note_id)?;
        let mut statement = self.connection.prepare(
            "SELECT library.id
             FROM note_library_refs refs
             JOIN notes note ON note.pk = refs.note_pk
             JOIN library_items library ON library.pk = refs.library_item_pk
             WHERE note.id = ?1
             ORDER BY library.id",
        )?;
        statement
            .query_map([note_id.to_string()], |row| row.get::<_, String>(0))?
            .map(|value| {
                value?.parse().map_err(|error: NtError| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                })
            })
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn notes_for_library(&self, library_id: &LibraryItemId) -> Result<Vec<NoteId>> {
        ensure_library_exists(&self.connection, library_id)?;
        let mut statement = self.connection.prepare(
            "SELECT note.id
             FROM note_library_refs refs
             JOIN notes note ON note.pk = refs.note_pk
             JOIN library_items library ON library.pk = refs.library_item_pk
             WHERE library.id = ?1
             ORDER BY note.id",
        )?;
        statement
            .query_map([library_id.to_string()], |row| row.get::<_, String>(0))?
            .map(|value| {
                value?.parse().map_err(|error: NtError| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                })
            })
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
    }
}

fn note_pk(transaction: &Transaction<'_>, id: &NoteId) -> Result<i64> {
    transaction
        .query_row(
            "SELECT pk FROM notes WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| NtError::NoteNotFound(id.to_string()))
}

fn library_pk(transaction: &Transaction<'_>, id: &LibraryItemId) -> Result<i64> {
    transaction
        .query_row(
            "SELECT pk FROM library_items WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| NtError::LibraryItemNotFound(id.to_string()))
}

#[cfg_attr(not(test), allow(dead_code))]
fn ensure_note_exists(connection: &rusqlite::Connection, id: &NoteId) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)",
        [id.to_string()],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(NtError::NoteNotFound(id.to_string()))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn ensure_library_exists(connection: &rusqlite::Connection, id: &LibraryItemId) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM library_items WHERE id = ?1)",
        [id.to_string()],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(NtError::LibraryItemNotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::library::{NewLibraryItem, Repository as LibraryRepository};
    use crate::domains::note::{CollectionPath, NewNote, Repository as NoteRepository};

    #[test]
    fn references_are_idempotent_many_to_many_and_cascade_from_both_sides() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        NoteRepository::initialize_at(&path).unwrap();
        let mut notes = NoteRepository::open_at(&path).unwrap();
        let note_a = notes
            .create_note(NewNote::new(CollectionPath::inbox(), "# A").unwrap())
            .unwrap();
        let note_b = notes
            .create_note(NewNote::new(CollectionPath::inbox(), "# B").unwrap())
            .unwrap();
        drop(notes);
        let mut library = LibraryRepository::open_at(&path).unwrap();
        let item_a = library
            .create_item(NewLibraryItem::new("a", "A", "content a").unwrap())
            .unwrap()
            .id()
            .clone();
        let item_b = library
            .create_item(NewLibraryItem::new("b", "B", "content b").unwrap())
            .unwrap()
            .id()
            .clone();
        drop(library);

        let mut refs = NoteLibraryRepository::open_at(&path).unwrap();
        assert!(refs.reference(&note_a, &item_a).unwrap());
        assert!(!refs.reference(&note_a, &item_a).unwrap());
        assert!(refs.reference(&note_a, &item_b).unwrap());
        assert!(refs.reference(&note_b, &item_a).unwrap());
        assert_eq!(
            refs.libraries_for_note(&note_a).unwrap(),
            vec![item_a.clone(), item_b.clone()]
        );
        assert_eq!(
            refs.notes_for_library(&item_a).unwrap(),
            vec![note_a.clone(), note_b.clone()]
        );
        drop(refs);

        NoteRepository::open_at(&path)
            .unwrap()
            .delete_notes(std::slice::from_ref(&note_a))
            .unwrap();
        let refs = NoteLibraryRepository::open_read_only(&path).unwrap();
        assert_eq!(refs.notes_for_library(&item_a).unwrap(), vec![note_b]);
        drop(refs);

        LibraryRepository::open_at(&path)
            .unwrap()
            .delete_item(&item_a)
            .unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM note_library_refs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn references_require_both_targets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        NoteRepository::initialize_at(&path).unwrap();
        let note: NoteId = "018fbe0a-6c00-7000-8000-000000000001".parse().unwrap();
        let item: LibraryItemId = "018fbe0a-6c00-7000-8000-000000000002".parse().unwrap();
        let mut refs = NoteLibraryRepository::open_at(&path).unwrap();
        assert!(matches!(
            refs.reference(&note, &item),
            Err(NtError::NoteNotFound(_))
        ));
    }
}
