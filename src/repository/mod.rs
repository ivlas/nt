use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::{NtError, Result};
use crate::fs::database_path;
use crate::note::Timestamp;

mod models;
mod notes;
mod schema;
mod search;

pub use models::{AgendaNote, FindRow, NoteChange, NoteMeta};

pub struct Repository {
    pub(super) connection: Connection,
}

pub(crate) struct InitRepository {
    repository: Option<Repository>,
    created: Option<CreatedDatabase>,
}

struct CreatedDatabase {
    path: PathBuf,
    remove_parent: bool,
}

impl Repository {
    pub fn open() -> Result<Self> {
        let path = database_path()?;
        let connection = open_existing(&path)?;
        if !is_nt_database(&connection, &path)? {
            return Err(NtError::UninitializedDatabase(path));
        }
        schema::configure_existing(&connection)?;
        let repository = Self { connection };
        if !repository.has_vault()? {
            return Err(NtError::MissingVault);
        }
        Ok(repository)
    }

    pub(crate) fn open_for_init() -> Result<InitRepository> {
        let path = database_path()?;
        match fs::metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(NtError::UninitializedDatabase(path));
                }
                Self::open_existing_for_init(path)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::create_for_init(path),
            Err(error) => Err(error.into()),
        }
    }

    fn open_existing_for_init(path: PathBuf) -> Result<InitRepository> {
        let connection = open_existing(&path)?;
        if !is_nt_database(&connection, &path)? {
            return Err(NtError::UninitializedDatabase(path));
        }
        let mut repository = Self { connection };
        schema::configure_and_initialize(&mut repository.connection)?;
        Ok(InitRepository {
            repository: Some(repository),
            created: None,
        })
    }

    fn create_for_init(path: PathBuf) -> Result<InitRepository> {
        let parent = path.parent().expect("database path always has a parent");
        let remove_parent = !parent.exists();
        fs::create_dir_all(parent)?;

        if let Err(error) = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            if error.kind() == io::ErrorKind::AlreadyExists {
                return Self::open_existing_for_init(path);
            }
            if remove_parent {
                let _ = fs::remove_dir(parent);
            }
            return Err(error.into());
        }

        let created = CreatedDatabase {
            path: path.clone(),
            remove_parent,
        };
        let result = (|| {
            let connection = open_existing(&path)?;
            let mut repository = Self { connection };
            schema::configure_and_initialize(&mut repository.connection)?;
            Ok(repository)
        })();

        match result {
            Ok(repository) => Ok(InitRepository {
                repository: Some(repository),
                created: Some(created),
            }),
            Err(error) => match created.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(cleanup_failed(error, cleanup_error)),
            },
        }
    }

    fn has_vault(&self) -> Result<bool> {
        self.connection
            .query_row("SELECT EXISTS(SELECT 1 FROM vaults)", [], |row| row.get(0))
            .map_err(Into::into)
    }
}

impl InitRepository {
    pub(crate) fn create_vault(mut self, name: &str, created_at: &Timestamp) -> Result<()> {
        let result = self
            .repository
            .as_mut()
            .expect("init repository is available")
            .create_vault(name, created_at);
        if result.is_ok() {
            self.created = None;
            return Ok(());
        }

        let error = result.expect_err("failed vault creation has an error");
        let created = self.created.take();
        drop(self.repository.take());
        match created.map(|created| created.cleanup()).transpose() {
            Ok(_) => Err(error),
            Err(cleanup_error) => Err(cleanup_failed(error, cleanup_error)),
        }
    }
}

impl Drop for InitRepository {
    fn drop(&mut self) {
        drop(self.repository.take());
        if let Some(created) = self.created.take() {
            let _ = created.cleanup();
        }
    }
}

impl CreatedDatabase {
    fn cleanup(self) -> io::Result<()> {
        for path in database_files(&self.path) {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if self.remove_parent
            && let Some(parent) = self.path.parent()
        {
            match fs::remove_dir(parent) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

fn open_existing(path: &Path) -> Result<Connection> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(NtError::UninitializedDatabase(path.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(NtError::MissingVault);
        }
        Err(error) => return Err(error.into()),
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(Into::into)
}

fn is_nt_database(connection: &Connection, path: &Path) -> Result<bool> {
    match schema::is_nt_database(connection) {
        Err(NtError::Database(source)) => Err(NtError::InvalidDatabase {
            path: path.to_path_buf(),
            source,
        }),
        result => result,
    }
}

fn database_files(path: &Path) -> Vec<PathBuf> {
    ["-wal", "-shm", "-journal", ""]
        .into_iter()
        .map(|suffix| {
            let mut value = path.as_os_str().to_os_string();
            value.push(suffix);
            PathBuf::from(value)
        })
        .collect()
}

fn cleanup_failed(error: NtError, cleanup_error: io::Error) -> NtError {
    NtError::Message(format!(
        "{error}; failed to clean up incomplete initialization: {cleanup_error}"
    ))
}
