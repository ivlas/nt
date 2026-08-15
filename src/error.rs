use thiserror::Error;

#[derive(Debug, Error)]
pub enum NtError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[source] rusqlite::Error),
    #[error("database is busy; retry")]
    DatabaseBusy,
    #[error("database is corrupt")]
    CorruptDatabase(#[source] rusqlite::Error),
    #[error("invalid database path")]
    InvalidDatabasePath,
    #[error("system clock is outside the supported timestamp range")]
    ClockOutOfRange,
    #[error("stored note has invalid body or title")]
    InvalidStoredNote,
    #[error("home directory not found")]
    HomeNotFound,
    #[error("run nt init first")]
    MissingDatabase,
    #[error("database is not an nt database")]
    NotNtDatabase,
    #[error("unsupported nt schema version {0}; delete ~/.nt/nt.sqlite3 and run nt init")]
    UnsupportedSchema(i64),
    #[error("invalid note id: {0}")]
    InvalidNoteId(String),
    #[error("invalid {field}: {value}")]
    InvalidValue { field: &'static str, value: String },
    #[error("body is empty")]
    EmptyBody,
    #[error("body must begin with '# <title>'")]
    InvalidTitle,
    #[error("cannot link note to itself")]
    SelfLink,
    #[error("invalid body version: {0}")]
    InvalidBodyVersion(u64),
    #[error("note not found: {0}")]
    NoteNotFound(String),
    #[error("duplicate note id: {0}")]
    DuplicateNoteId(String),
    #[error("cannot combine body arguments with stdin")]
    ConflictingBodyInput,
    #[error("VISUAL or EDITOR is not set")]
    EditorNotSet,
    #[error("invalid VISUAL or EDITOR command")]
    InvalidEditor,
    #[error("editor exited unsuccessfully")]
    EditorFailed,
    #[error("note changed while editing: {0}")]
    ConcurrentEdit(String),
}

impl From<rusqlite::Error> for NtError {
    fn from(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(sqlite_error, _) = &error {
            match sqlite_error.code {
                rusqlite::ffi::ErrorCode::DatabaseBusy
                | rusqlite::ffi::ErrorCode::DatabaseLocked => return Self::DatabaseBusy,
                rusqlite::ffi::ErrorCode::DatabaseCorrupt
                | rusqlite::ffi::ErrorCode::NotADatabase => {
                    return Self::CorruptDatabase(error);
                }
                _ => {}
            }
        }
        Self::Database(error)
    }
}

pub type Result<T> = std::result::Result<T, NtError>;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::NtError;

    #[test]
    fn sqlite_failures_keep_stable_operational_categories() {
        for code in [rusqlite::ffi::SQLITE_BUSY, rusqlite::ffi::SQLITE_LOCKED] {
            assert!(matches!(
                NtError::from(sqlite_failure(code)),
                NtError::DatabaseBusy
            ));
        }
        for code in [rusqlite::ffi::SQLITE_CORRUPT, rusqlite::ffi::SQLITE_NOTADB] {
            let error = NtError::from(sqlite_failure(code));
            assert!(matches!(error, NtError::CorruptDatabase(_)));
            assert_eq!(error.to_string(), "database is corrupt");
            assert!(error.source().is_some());
        }

        assert!(matches!(
            NtError::from(sqlite_failure(rusqlite::ffi::SQLITE_IOERR)),
            NtError::Database(_)
        ));
    }

    fn sqlite_failure(code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
    }
}
