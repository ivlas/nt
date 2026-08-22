use rusqlite::TransactionBehavior;

use super::super::super::{CollectionPath, NewNote, NoteId, NoteQuery};
use super::super::{AddOrRemove, Repository};
use super::{repository, summaries};
use crate::error::{NtError, Result};
use crate::schema;
use crate::storage::{InitOutcome, OpenMode};

fn initialize_at(path: &std::path::Path) -> Result<InitOutcome> {
    schema::initialize_at(path)
}

fn open_at(path: &std::path::Path, mode: OpenMode) -> Result<Repository> {
    let connection = match mode {
        OpenMode::ReadOnly => schema::open_read_only(path),
        OpenMode::ReadWrite => schema::open_read_write(path),
    }?;
    Ok(Repository::from_connection(connection))
}

#[test]
fn creates_loads_lists_and_deletes_notes() {
    let mut repository = repository();
    let id = repository
        .create_note(
            NewNote::new(CollectionPath::inbox(), "# Storage\nBody")
                .unwrap()
                .with_tags(["rust".parse().unwrap()]),
        )
        .unwrap();
    let note = repository.get_note(&id).unwrap();
    assert_eq!(note.body(), "# Storage\nBody");

    let notes = summaries(&repository, &NoteQuery::default());
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id(), &id);
    assert_eq!(notes[0].tags().len(), 1);
    repository.delete_notes(std::slice::from_ref(&id)).unwrap();
    assert!(matches!(
        repository.get_note(&id),
        Err(NtError::NoteNotFound(_))
    ));
}

#[test]
fn complete_note_load_uses_one_read_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_at(&path).unwrap();
    let mut writer = open_at(&path, OpenMode::ReadWrite).unwrap();
    let target = writer
        .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
        .unwrap();
    let source = writer
        .create_note(
            NewNote::new(CollectionPath::inbox(), "# Source")
                .unwrap()
                .with_tags(["old".parse().unwrap()])
                .with_links([target.clone()]),
        )
        .unwrap();
    let mut reader = open_at(&path, OpenMode::ReadWrite).unwrap();
    let expected = reader.get_note(&source).unwrap();

    let transaction = reader
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .unwrap();
    transaction
        .query_row(
            "SELECT 1 FROM notes WHERE id = ?1",
            [source.to_string()],
            |_| Ok(()),
        )
        .unwrap();
    writer
        .change_tag(&source, AddOrRemove::Remove("old".parse().unwrap()))
        .unwrap();
    writer
        .change_tag(&source, AddOrRemove::Add("new".parse().unwrap()))
        .unwrap();
    writer.delete_notes(std::slice::from_ref(&target)).unwrap();

    assert_eq!(
        super::super::store::load_note(&transaction, &source).unwrap(),
        expected
    );
    transaction.commit().unwrap();
    assert_ne!(reader.get_note(&source).unwrap(), expected);
}

#[test]
fn validates_link_targets_and_atomic_deletion() {
    let mut repository = repository();
    let missing: NoteId = "018fbe0a-6c00-7000-8000-000000000001".parse().unwrap();
    let result = repository.create_note(
        NewNote::new(CollectionPath::inbox(), "# Link")
            .unwrap()
            .with_links([missing.clone()]),
    );
    assert!(matches!(result, Err(NtError::NoteNotFound(_))));

    let first = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# First").unwrap())
        .unwrap();
    let result = repository.delete_notes(&[first.clone(), missing]);
    assert!(matches!(result, Err(NtError::NoteNotFound(_))));
    assert!(repository.get_note(&first).is_ok());
}

#[test]
fn duplicate_deletion_is_rejected_without_deleting_the_note() {
    let mut repository = repository();
    let id = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Kept").unwrap())
        .unwrap();

    let result = repository.delete_notes(&[id.clone(), id.clone()]);

    assert!(matches!(
        result,
        Err(NtError::DuplicateNoteId(duplicate)) if duplicate == id.to_string()
    ));
    assert!(repository.get_note(&id).is_ok());
}
