use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::error::{NtError, Result};
#[cfg(test)]
mod behavior_tests;
mod note_store;
mod query_sql;
mod relationships;
mod schema;
mod summaries;

pub use summaries::NoteSummary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddOrRemove<T> {
    Add(T),
    Remove(T),
}

impl<T: fmt::Display> fmt::Display for AddOrRemove<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add(value) => write!(formatter, "+{value}"),
            Self::Remove(value) => write!(formatter, "-{value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitOutcome {
    Initialized,
    AlreadyInitialized,
}

pub struct Repository {
    pub(super) connection: Connection,
}

impl Repository {
    pub fn initialize_at(path: &Path) -> Result<InitOutcome> {
        initialize_at(path)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        open_at(path)
    }
}

fn initialize_at(path: &Path) -> Result<InitOutcome> {
    create_empty_if_missing(path)?;
    let mut connection = open_existing(path)?;
    let outcome = if schema::initialize(&mut connection)? {
        InitOutcome::Initialized
    } else {
        InitOutcome::AlreadyInitialized
    };
    schema::configure(&connection)?;
    Ok(outcome)
}

fn open_at(path: &Path) -> Result<Repository> {
    let connection = open_existing(path)?;
    match schema::inspect(&connection)? {
        schema::Identity::Nt => {}
        schema::Identity::Empty => return Err(NtError::NotNtDatabase),
    }
    schema::configure(&connection)?;
    Ok(Repository { connection })
}

fn create_empty_if_missing(path: &Path) -> Result<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => return Ok(()),
        Ok(_) => return Err(NtError::NotNtDatabase),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let parent = path.parent().expect("database path has a parent");
    fs::create_dir_all(parent)?;
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn open_existing(path: &Path) -> Result<Connection> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(NtError::NotNtDatabase),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(NtError::MissingDatabase);
        }
        Err(error) => return Err(error.into()),
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use rusqlite::Connection;

    use super::*;
    use crate::note::{CollectionPath, NewNote};

    #[test]
    fn initializes_and_reopens_a_clean_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        assert_eq!(
            initialize_at(&path).unwrap(),
            InitOutcome::AlreadyInitialized
        );
        let repository = open_at(&path).unwrap();
        let foreign_keys: i64 = repository
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let journal: String = repository
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal, "wal");
    }

    #[test]
    fn ordinary_open_does_not_create_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        assert!(matches!(open_at(&path), Err(NtError::MissingDatabase)));
        assert!(!path.exists());
    }

    #[test]
    fn initialization_accepts_zero_length_and_empty_sqlite_files() {
        for name in ["zero.sqlite3", "empty.sqlite3"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(name);
            if name == "zero.sqlite3" {
                fs::write(&path, []).unwrap();
            } else {
                drop(Connection::open(&path).unwrap());
            }
            assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
            assert!(open_at(&path).is_ok());
        }
    }

    #[test]
    fn unrelated_databases_are_rejected_without_modification() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("other.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE other(value TEXT)")
            .unwrap();
        drop(connection);
        let before = fs::read(&path).unwrap();
        assert!(matches!(initialize_at(&path), Err(NtError::NotNtDatabase)));
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn corrupt_database_errors_are_not_reported_as_foreign_schema() {
        for operation in [initialize_at as fn(&Path) -> Result<InitOutcome>, |path| {
            open_at(path).map(|_| InitOutcome::AlreadyInitialized)
        }] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("corrupt.sqlite3");
            fs::write(&path, b"not a sqlite database").unwrap();

            assert!(matches!(operation(&path), Err(NtError::CorruptDatabase(_))));
        }
    }

    #[test]
    fn incompatible_nt_versions_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON; UPDATE schema_version SET version = 2",
            )
            .unwrap();
        drop(connection);
        assert!(matches!(open_at(&path), Err(NtError::UnsupportedSchema(2))));
    }

    #[test]
    fn another_application_id_is_rejected_without_schema_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("other.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA application_id = 1234; CREATE TABLE other(value TEXT)")
            .unwrap();
        drop(connection);
        assert!(matches!(initialize_at(&path), Err(NtError::NotNtDatabase)));
        let connection = Connection::open(path).unwrap();
        let application_id: i64 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .unwrap();
        assert_eq!(application_id, 1234);
        let table_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'other')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1);
    }

    #[test]
    fn writer_contention_returns_the_retryable_busy_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        initialize_at(&path).unwrap();
        let mut first = open_at(&path).unwrap();
        let mut second = open_at(&path).unwrap();
        second
            .connection
            .busy_timeout(Duration::from_millis(1))
            .unwrap();
        let transaction = first
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let result =
            second.create_note(NewNote::new(CollectionPath::inbox(), "# Contended").unwrap());
        assert!(matches!(result, Err(NtError::DatabaseBusy)));
        transaction.rollback().unwrap();
    }

    #[test]
    fn concurrent_initializers_preserve_the_winning_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    initialize_at(&path)
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == InitOutcome::Initialized)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == InitOutcome::AlreadyInitialized)
                .count(),
            7
        );
        assert!(open_at(&path).is_ok());
    }
}
