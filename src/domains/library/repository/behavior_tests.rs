use std::sync::{Arc, Barrier};

use super::super::{LibraryQuery, NewLibraryCapture, NewLibraryItem};
use super::Repository;
use super::store::hash_for_test;
use crate::error::NtError;

fn repository() -> Repository {
    let mut connection = rusqlite::Connection::open_in_memory().unwrap();
    crate::schema::initialize(&mut connection).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .unwrap();
    Repository { connection }
}

fn initialize_path(path: &std::path::Path) {
    let mut connection = rusqlite::Connection::open(path).unwrap();
    crate::schema::initialize(&mut connection).unwrap();
    connection
        .execute_batch("PRAGMA journal_mode = WAL")
        .unwrap();
}

#[test]
fn creation_is_atomic_and_duplicate_sources_append_only_new_content() {
    let mut repository = repository();
    let first = repository
        .create_item(
            NewLibraryItem::new("https://example.com", "Original title", "first content").unwrap(),
        )
        .unwrap();
    assert!(first.item_created());
    assert!(first.capture_created());

    let duplicate = repository
        .create_item(
            NewLibraryItem::new("https://example.com", "Ignored title", "first content").unwrap(),
        )
        .unwrap();
    assert_eq!(duplicate.id(), first.id());
    assert!(!duplicate.item_created());
    assert!(!duplicate.capture_created());
    assert_eq!(
        repository.get_item(first.id()).unwrap().title(),
        "Original title"
    );
    assert!(
        repository
            .update_title(first.id(), "Explicit title")
            .unwrap()
    );
    assert_eq!(
        repository.get_item(first.id()).unwrap().title(),
        "Explicit title"
    );

    let changed = repository
        .create_item(
            NewLibraryItem::new("https://example.com", "Ignored title", "second content").unwrap(),
        )
        .unwrap();
    assert!(changed.capture_created());
    let history = repository.history(first.id()).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].capture().content(), "first content");
    assert_eq!(history[1].capture().content(), "second content");
}

#[test]
fn failed_first_capture_rolls_back_the_item_and_fts() {
    let mut repository = repository();
    repository
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_library_capture BEFORE INSERT ON library_captures BEGIN
             SELECT RAISE(ABORT, 'injected failure');
         END",
        )
        .unwrap();
    assert!(
        repository
            .create_item(NewLibraryItem::new("rollback", "Rollback", "content").unwrap(),)
            .is_err()
    );
    let items: i64 = repository
        .connection
        .query_row("SELECT COUNT(*) FROM library_items", [], |row| row.get(0))
        .unwrap();
    let captures: i64 = repository
        .connection
        .query_row("SELECT COUNT(*) FROM library_captures", [], |row| {
            row.get(0)
        })
        .unwrap();
    let fts: i64 = repository
        .connection
        .query_row("SELECT COUNT(*) FROM library_capture_fts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((items, captures, fts), (0, 0, 0));
}

#[test]
fn captures_hash_exact_bytes_with_blake3_and_select_latest_deterministically() {
    let mut repository = repository();
    let outcome = repository
        .create_item(NewLibraryItem::new("source", "Title", "line\n").unwrap())
        .unwrap();
    repository
        .capture(outcome.id(), NewLibraryCapture::new("line").unwrap())
        .unwrap();

    let history = repository.history(outcome.id()).unwrap();
    assert_eq!(history[0].capture().content_hash(), hash_for_test("line\n"));
    assert_ne!(hash_for_test("line\n"), hash_for_test("line"));
    assert_eq!(
        repository
            .get_latest_capture(outcome.id())
            .unwrap()
            .content(),
        "line"
    );
}

#[test]
fn summaries_replace_per_capture_and_never_follow_new_content() {
    let mut repository = repository();
    let outcome = repository
        .create_item(NewLibraryItem::new("source", "Title", "capture one").unwrap())
        .unwrap();
    repository
        .replace_latest_summary(outcome.id(), "summary one", "manual", "1")
        .unwrap();
    repository
        .replace_latest_summary(outcome.id(), "summary replacement", "manual", "2")
        .unwrap();
    repository
        .capture(outcome.id(), NewLibraryCapture::new("capture two").unwrap())
        .unwrap();

    let history = repository.history(outcome.id()).unwrap();
    assert_eq!(
        history[0].summary().unwrap().summary(),
        "summary replacement"
    );
    assert_eq!(history[0].summary().unwrap().version(), "2");
    assert!(history[1].summary().is_none());
}

#[test]
fn default_search_uses_only_current_capture_with_unicode_fts_semantics() {
    let mut repository = repository();
    let outcome = repository
        .create_item(NewLibraryItem::new("source", "Title", "Café historical").unwrap())
        .unwrap();
    repository
        .capture(
            outcome.id(),
            NewLibraryCapture::new("Current résumé storage").unwrap(),
        )
        .unwrap();

    for (term, expected) in [("resume", 1), ("STORAGE", 1), ("historical", 0)] {
        let query = LibraryQuery::parse_find(&[term.to_string()]).unwrap();
        let mut rows = 0;
        repository
            .visit_summaries(&query, |_| {
                rows += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(rows, expected, "term {term}");
    }
}

#[test]
fn rolled_back_capture_leaves_fts_unchanged_and_item_deletion_cleans_it() {
    let mut repository = repository();
    let outcome = repository
        .create_item(NewLibraryItem::new("source", "Title", "committed").unwrap())
        .unwrap();
    let item_pk: i64 = repository
        .connection
        .query_row(
            "SELECT pk FROM library_items WHERE id = ?1",
            [outcome.id().to_string()],
            |row| row.get(0),
        )
        .unwrap();
    let transaction = repository.connection.transaction().unwrap();
    transaction
        .execute(
            "INSERT INTO library_captures(item_pk, captured, content, content_hash)
         VALUES (?1, '2026-01-01T00:00:00Z', 'rolledback', ?2)",
            rusqlite::params![item_pk, hash_for_test("rolledback")],
        )
        .unwrap();
    transaction.rollback().unwrap();
    let rolled_back: i64 = repository
        .connection
        .query_row(
            "SELECT COUNT(*) FROM library_capture_fts WHERE library_capture_fts MATCH 'rolledback'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rolled_back, 0);

    repository.delete_item(outcome.id()).unwrap();
    repository
        .connection
        .execute(
            "INSERT INTO library_capture_fts(library_capture_fts) VALUES ('integrity-check')",
            [],
        )
        .unwrap();
    let count: i64 = repository
        .connection
        .query_row("SELECT COUNT(*) FROM library_capture_fts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn concurrent_same_source_and_content_produce_one_item_and_capture() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_path(&path);
    let barrier = Arc::new(Barrier::new(4));
    let handles = (0..4)
        .map(|_| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut repository = Repository::open_at(&path).unwrap();
                barrier.wait();
                repository.create_item(
                    NewLibraryItem::new("same-source", "Title", "same content").unwrap(),
                )
            })
        })
        .collect::<Vec<_>>();
    let outcomes = handles
        .into_iter()
        .map(|handle| handle.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.id() == outcomes[0].id())
    );
    let repository = Repository::open_read_only(&path).unwrap();
    assert_eq!(repository.history(outcomes[0].id()).unwrap().len(), 1);
}

#[test]
fn concurrent_distinct_captures_are_both_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_path(&path);
    let mut repository = Repository::open_at(&path).unwrap();
    let id = repository
        .create_item(NewLibraryItem::new("source", "Title", "initial").unwrap())
        .unwrap()
        .id()
        .clone();
    drop(repository);

    let barrier = Arc::new(Barrier::new(2));
    let handles = ["capture a", "capture b"].map(|content| {
        let path = path.clone();
        let id = id.clone();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            let mut repository = Repository::open_at(&path).unwrap();
            barrier.wait();
            repository.capture(&id, NewLibraryCapture::new(content).unwrap())
        })
    });
    for handle in handles {
        assert!(handle.join().unwrap().unwrap());
    }
    let history = Repository::open_read_only(&path)
        .unwrap()
        .history(&id)
        .unwrap();
    assert_eq!(history.len(), 3);
    assert!(
        history
            .iter()
            .any(|row| row.capture().content() == "capture a")
    );
    assert!(
        history
            .iter()
            .any(|row| row.capture().content() == "capture b")
    );
}

#[test]
fn readers_exclude_uncommitted_captures_and_busy_writers_use_retryable_error() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_path(&path);
    let mut first = Repository::open_at(&path).unwrap();
    let id = first
        .create_item(NewLibraryItem::new("source", "Title", "committed").unwrap())
        .unwrap()
        .id()
        .clone();
    let mut second = Repository::open_at(&path).unwrap();
    second
        .connection
        .busy_timeout(std::time::Duration::from_millis(1))
        .unwrap();

    let transaction = first
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let item_pk: i64 = transaction
        .query_row(
            "SELECT pk FROM library_items WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO library_captures(item_pk, captured, content, content_hash)
             VALUES (?1, '2026-01-01T00:00:00Z', 'uncommitted', ?2)",
            rusqlite::params![item_pk, hash_for_test("uncommitted")],
        )
        .unwrap();

    let reader = Repository::open_read_only(&path).unwrap();
    assert_eq!(reader.history(&id).unwrap().len(), 1);
    assert!(matches!(
        second.capture(&id, NewLibraryCapture::new("contended").unwrap()),
        Err(NtError::DatabaseBusy)
    ));
    transaction.rollback().unwrap();
}

#[test]
fn read_only_repository_rejects_mutation_and_missing_items_are_stable_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nt.sqlite3");
    initialize_path(&path);
    let mut writer = Repository::open_at(&path).unwrap();
    let outcome = writer
        .create_item(NewLibraryItem::new("source", "Title", "body").unwrap())
        .unwrap();
    drop(writer);
    let mut reader = Repository::open_read_only(&path).unwrap();
    assert!(reader.get_latest_capture(outcome.id()).is_ok());
    assert!(
        reader
            .capture(outcome.id(), NewLibraryCapture::new("denied").unwrap())
            .is_err()
    );

    let missing = "018fbe0a-6c00-7000-8000-000000000001".parse().unwrap();
    assert!(matches!(
        reader.get_item(&missing),
        Err(NtError::LibraryItemNotFound(_))
    ));
}
