use std::sync::{Arc, Barrier};

use rusqlite::TransactionBehavior;

use super::super::super::{CollectionPath, NewNote, NoteId};
use super::super::store::next_revision;
use super::super::{AddOrRemove, Repository};
use super::repository;
use crate::error::NtError;
use crate::schema;

fn current(repository: &Repository) -> i64 {
    repository
        .connection
        .query_row(
            "SELECT revision FROM global_revision WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn real_mutations_are_strictly_monotonic_and_noops_and_failures_do_not_advance() {
    let mut repository = repository();
    assert_eq!(current(&repository), 0);

    let source = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
        .unwrap();
    assert_eq!(
        (
            current(&repository),
            repository.get_note(&source).unwrap().revision()
        ),
        (1, 1)
    );

    assert!(
        repository
            .change_tag(&source, AddOrRemove::Add("rust".parse().unwrap()))
            .unwrap()
    );
    assert_eq!(
        (
            current(&repository),
            repository.get_note(&source).unwrap().revision()
        ),
        (2, 2)
    );
    assert!(
        !repository
            .change_tag(&source, AddOrRemove::Add("rust".parse().unwrap()))
            .unwrap()
    );
    assert!(
        !repository
            .move_note(&source, &CollectionPath::inbox())
            .unwrap()
    );
    repository.verify_body_version(&source, 1).unwrap();
    assert_eq!(current(&repository), 2);

    let mut edited = repository.get_note(&source).unwrap();
    edited
        .replace_body("# Edited", "2026-08-25T12:00:00Z".parse().unwrap())
        .unwrap();
    repository.replace_body(&edited, 1).unwrap();
    assert_eq!(
        (
            current(&repository),
            repository.get_note(&source).unwrap().revision()
        ),
        (3, 3)
    );
    assert!(repository.replace_body(&edited, 1).is_err());
    assert_eq!(current(&repository), 3);

    let missing: NoteId = "018fbe0a-6c00-7000-8000-000000000099".parse().unwrap();
    let failed = NewNote::new(CollectionPath::inbox(), "# Failed")
        .unwrap()
        .with_links([missing.clone()]);
    assert!(repository.create_note(failed).is_err());
    assert!(repository.delete_notes(&[missing]).is_err());
    assert_eq!(current(&repository), 3);

    let target = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
        .unwrap();
    assert_eq!(current(&repository), 4);
    assert!(
        repository
            .change_link(&source, AddOrRemove::Add(target.clone()))
            .unwrap()
    );
    assert_eq!(current(&repository), 5);
    repository.delete_notes(&[target]).unwrap();
    assert_eq!(
        (
            current(&repository),
            repository.get_note(&source).unwrap().revision()
        ),
        (6, 6)
    );
    repository.delete_notes(&[]).unwrap();
    assert_eq!(current(&repository), 6);
    repository.delete_notes(&[source]).unwrap();
    assert_eq!(current(&repository), 7);
}

#[test]
fn rolled_back_allocations_are_not_observable() {
    let mut repository = repository();
    let transaction = repository
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(next_revision(&transaction).unwrap(), 1);
    transaction.rollback().unwrap();
    assert_eq!(current(&repository), 0);

    let id = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Committed").unwrap())
        .unwrap();
    assert_eq!(
        (
            current(&repository),
            repository.get_note(&id).unwrap().revision()
        ),
        (1, 1)
    );
}

#[test]
fn concurrent_writers_assign_unique_commit_ordered_revisions_that_survive_reopen() {
    const WRITERS: usize = 8;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    schema::initialize_at(&path).unwrap();
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut threads = Vec::new();

    for writer in 0..WRITERS {
        let path = path.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            let connection = schema::open_read_write(&path).unwrap();
            let mut repository = Repository::from_connection(connection);
            barrier.wait();
            repository
                .create_note(
                    NewNote::new(CollectionPath::inbox(), format!("# Writer {writer}")).unwrap(),
                )
                .unwrap();
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }

    let connection = schema::open_read_only(&path).unwrap();
    let current: i64 = connection
        .query_row("SELECT revision FROM global_revision", [], |row| row.get(0))
        .unwrap();
    let revisions = connection
        .prepare("SELECT note_revision FROM notes ORDER BY note_revision")
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(current, WRITERS as i64);
    assert_eq!(revisions, (1..=WRITERS as i64).collect::<Vec<_>>());
    let repository = Repository::from_connection(connection);
    let mut change_revisions = Vec::new();
    repository
        .visit_changes_since(0, |change| {
            change_revisions.push(change.revision());
            Ok(())
        })
        .unwrap();
    assert_eq!(change_revisions, (1..=WRITERS as u64).collect::<Vec<_>>());
    drop(repository);

    let reopened = schema::open_read_only(&path).unwrap();
    assert_eq!(
        reopened
            .query_row("SELECT revision FROM global_revision", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        WRITERS as i64
    );
}

#[test]
fn simultaneous_conditional_writers_allow_exactly_one_observed_revision() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    schema::initialize_at(&path).unwrap();
    let id = Repository::from_connection(schema::open_read_write(&path).unwrap())
        .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut threads = Vec::new();

    for tag in ["first", "second"] {
        let path = path.clone();
        let id = id.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            let mut repository =
                Repository::from_connection(schema::open_read_write(&path).unwrap());
            barrier.wait();
            repository.change_tag_if_revision(&id, AddOrRemove::Add(tag.parse().unwrap()), Some(1))
        }));
    }

    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(NtError::RevisionConflict(_))))
            .count(),
        1
    );
    let repository = Repository::from_connection(schema::open_read_only(&path).unwrap());
    let note = repository.get_note(&id).unwrap();
    assert_eq!(note.revision(), 2);
    assert_eq!(note.tags().len(), 1);
    assert_eq!(current(&repository), 2);
}
