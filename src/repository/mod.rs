use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::{NtError, Result};
use crate::fs::database_path;

mod schema;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitOutcome {
    Initialized,
    AlreadyInitialized,
}

pub struct Repository {
    pub(super) connection: Connection,
}

impl Repository {
    pub fn initialize() -> Result<InitOutcome> {
        initialize_at(&database_path()?)
    }

    pub fn open() -> Result<Self> {
        open_at(&database_path()?)
    }
}

fn initialize_at(path: &Path) -> Result<InitOutcome> {
    let created = create_empty_if_missing(path)?;
    let result = (|| {
        let mut connection = open_existing(path)?;
        let outcome = match inspect(&connection)? {
            schema::Identity::Empty => {
                schema::initialize(&mut connection)?;
                InitOutcome::Initialized
            }
            schema::Identity::Nt => InitOutcome::AlreadyInitialized,
        };
        schema::configure(&connection)?;
        Ok(outcome)
    })();

    if result.is_err() && created {
        cleanup_created_database(path);
    }
    result
}

fn open_at(path: &Path) -> Result<Repository> {
    let connection = open_existing(path)?;
    match inspect(&connection)? {
        schema::Identity::Nt => {}
        schema::Identity::Empty => return Err(NtError::NotNtDatabase),
    }
    schema::configure(&connection)?;
    Ok(Repository { connection })
}

fn inspect(connection: &Connection) -> Result<schema::Identity> {
    match schema::inspect(connection) {
        Err(NtError::Database(_)) => Err(NtError::NotNtDatabase),
        result => result,
    }
}

fn create_empty_if_missing(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => return Ok(false),
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
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
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

fn cleanup_created_database(path: &Path) {
    for suffix in ["-wal", "-shm", "-journal", ""] {
        let mut file = path.as_os_str().to_os_string();
        file.push(suffix);
        let _ = fs::remove_file(PathBuf::from(file));
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

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
}
