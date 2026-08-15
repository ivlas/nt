use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use rusqlite::Connection;

use crate::error::{NtError, Result};
#[cfg(test)]
mod behavior_tests;
mod connection;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenMode {
    ReadOnly,
    ReadWrite,
}

pub struct Repository {
    pub(super) connection: Connection,
}

struct RemoveEmptyDirectory<'a>(Option<&'a Path>);

impl RemoveEmptyDirectory<'_> {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for RemoveEmptyDirectory<'_> {
    fn drop(&mut self) {
        if let Some(path) = self.0 {
            let _ = fs::remove_dir(path);
        }
    }
}

impl Repository {
    pub fn initialize_at(path: &Path) -> Result<InitOutcome> {
        initialize_at(path)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        open_at(path, OpenMode::ReadWrite)
    }

    pub fn open_read_only(path: &Path) -> Result<Self> {
        open_at(path, OpenMode::ReadOnly)
    }
}

fn initialize_at(path: &Path) -> Result<InitOutcome> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => initialize_existing(path, true),
        Ok(_) => Err(NtError::NotNtDatabase),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            initialize_missing_with(path, schema::initialize)
        }
        Err(error) => Err(error.into()),
    }
}

fn initialize_existing(path: &Path, establish_wal: bool) -> Result<InitOutcome> {
    let mut connection = connection::open_existing(path, OpenMode::ReadWrite)?;
    connection::configure(&connection)?;
    let outcome = if schema::initialize(&mut connection)? {
        InitOutcome::Initialized
    } else {
        InitOutcome::AlreadyInitialized
    };
    if establish_wal || outcome == InitOutcome::Initialized {
        connection::configure_wal(&connection)?;
    }
    Ok(outcome)
}

fn initialize_missing_with(
    path: &Path,
    initialize: impl FnOnce(&mut Connection) -> Result<bool>,
) -> Result<InitOutcome> {
    let parent = path.parent().ok_or(NtError::InvalidDatabasePath)?;
    let parent_existed = parent.exists();
    fs::create_dir_all(parent)?;
    let mut parent_cleanup = RemoveEmptyDirectory((!parent_existed).then_some(parent));
    let candidate = tempfile::NamedTempFile::new_in(parent)?;
    let mut connection = connection::open_existing(candidate.path(), OpenMode::ReadWrite)?;
    connection::configure(&connection)?;
    if !initialize(&mut connection)? {
        return Err(NtError::NotNtDatabase);
    }
    drop(connection);

    match candidate.persist_noclobber(path) {
        Ok(file) => {
            drop(file);
            parent_cleanup.disarm();
            let connection = connection::open_existing(path, OpenMode::ReadWrite)?;
            connection::configure_wal(&connection)?;
            Ok(InitOutcome::Initialized)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            drop(error.file);
            parent_cleanup.disarm();
            initialize_existing(path, false)
        }
        Err(error) => Err(error.error.into()),
    }
}

fn open_at(path: &Path, mode: OpenMode) -> Result<Repository> {
    let connection = connection::open_existing(path, mode)?;
    match schema::inspect(&connection)? {
        schema::Identity::Nt => {}
        schema::Identity::Empty => return Err(NtError::NotNtDatabase),
    }
    match mode {
        OpenMode::ReadOnly => connection::configure(&connection)?,
        OpenMode::ReadWrite => connection::configure_wal(&connection)?,
    }
    Ok(Repository { connection })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use rusqlite::Connection;

    use super::*;
    use crate::note::{CollectionPath, NewNote};
    use crate::query::NoteQuery;

    #[test]
    fn initializes_and_reopens_a_clean_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        assert_eq!(
            initialize_at(&path).unwrap(),
            InitOutcome::AlreadyInitialized
        );
        let repository = open_at(&path, OpenMode::ReadWrite).unwrap();
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
        assert!(matches!(
            open_at(&path, OpenMode::ReadWrite),
            Err(NtError::MissingDatabase)
        ));
        assert!(!path.exists());
    }

    #[test]
    fn failed_new_initialization_leaves_no_database_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");

        let result = initialize_missing_with(&path, |connection| {
            schema::initialize_with(connection, |step| {
                if step == 3 {
                    return Err(NtError::Io(io::Error::other(
                        "injected initialization failure",
                    )));
                }
                Ok(())
            })?;
            Ok(true)
        });

        assert!(result.is_err());
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
    }

    #[test]
    fn read_only_connections_read_notes_and_reject_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        initialize_at(&path).unwrap();
        let id = {
            let mut writer = open_at(&path, OpenMode::ReadWrite).unwrap();
            writer
                .create_note(
                    NewNote::new(CollectionPath::inbox(), "# Read only")
                        .unwrap()
                        .with_tags(["rust".parse().unwrap()]),
                )
                .unwrap()
        };

        let reader = open_at(&path, OpenMode::ReadOnly).unwrap();
        let foreign_keys: i64 = reader
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let journal: String = reader
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!((foreign_keys, journal.as_str()), (1, "wal"));
        drop(reader);

        let mut reader = open_at(&path, OpenMode::ReadOnly).unwrap();
        assert_eq!(reader.get_note(&id).unwrap().body(), "# Read only");
        assert_eq!(reader.list_tags().unwrap(), vec!["rust".parse().unwrap()]);
        let mut visited = 0;
        reader
            .visit_note_summaries(&NoteQuery::default(), |_| {
                visited += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(visited, 1);

        assert!(
            reader
                .create_note(NewNote::new(CollectionPath::inbox(), "# Denied").unwrap())
                .is_err()
        );
    }

    #[test]
    fn initialization_rejects_paths_without_a_parent() {
        assert!(matches!(
            initialize_at(Path::new("")),
            Err(NtError::InvalidDatabasePath)
        ));
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
            assert!(open_at(&path, OpenMode::ReadWrite).is_ok());
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
            open_at(path, OpenMode::ReadWrite).map(|_| InitOutcome::AlreadyInitialized)
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
        assert!(matches!(
            open_at(&path, OpenMode::ReadWrite),
            Err(NtError::UnsupportedSchema(2))
        ));
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
        let mut first = open_at(&path, OpenMode::ReadWrite).unwrap();
        let mut second = open_at(&path, OpenMode::ReadWrite).unwrap();
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
        assert!(open_at(&path, OpenMode::ReadWrite).is_ok());
    }
}
