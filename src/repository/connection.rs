use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{NtError, Result};

use super::OpenMode;

pub(super) fn open_existing(path: &Path, mode: OpenMode) -> Result<Connection> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(NtError::NotNtDatabase),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(NtError::MissingDatabase);
        }
        Err(error) => return Err(error.into()),
    }
    let flags = match mode {
        OpenMode::ReadOnly => OpenFlags::SQLITE_OPEN_READ_ONLY,
        OpenMode::ReadWrite => OpenFlags::SQLITE_OPEN_READ_WRITE,
    };
    Connection::open_with_flags(path, flags).map_err(Into::into)
}

pub(super) fn configure(connection: &Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(())
}

pub(super) fn configure_wal(connection: &Connection) -> Result<()> {
    configure(connection)?;
    connection.execute_batch("PRAGMA journal_mode = WAL")?;
    Ok(())
}
