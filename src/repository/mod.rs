use std::fs;
use std::path::Path;

use rusqlite::Connection;

use crate::error::{NtError, Result};
use crate::fs::database_path;

mod models;
mod notes;
mod schema;
mod search;

pub use models::{AgendaNote, FindRow, NoteChange, NoteMeta};
pub use notes::parse_collection_name;

pub struct Repository {
    pub(super) connection: Connection,
}

impl Repository {
    pub fn open() -> Result<Self> {
        let path = database_path()?;
        Self::open_path(&path)
    }

    pub fn open_for_init() -> Result<Self> {
        let path = database_path()?;
        let existed = path.exists();
        let repository = Self::open_path_uninitialized(&path)?;
        if existed && !schema::is_nt_database(&repository.connection)? {
            return Err(NtError::Message(format!(
                "database already exists at {}; refusing to overwrite it",
                path.display()
            )));
        }
        schema::configure_and_initialize(&repository.connection)?;
        Ok(repository)
    }

    fn open_path(path: &Path) -> Result<Self> {
        let repository = Self::open_path_uninitialized(path)?;
        schema::configure_and_initialize(&repository.connection)?;
        Ok(repository)
    }

    fn open_path_uninitialized(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        Ok(Self { connection })
    }
}
