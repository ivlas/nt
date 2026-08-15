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
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if journal_mode.eq_ignore_ascii_case("wal") {
        Ok(())
    } else {
        Err(NtError::WalUnavailable)
    }
}
