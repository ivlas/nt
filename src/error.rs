use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredNoteContext {
    pub(crate) note_id: Option<String>,
    pub(crate) row_id: Option<i64>,
}

impl StoredNoteContext {
    pub(crate) fn new(note_id: Option<String>, row_id: Option<i64>) -> Self {
        Self { note_id, row_id }
    }
}

impl fmt::Display for StoredNoteContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.note_id, self.row_id) {
            (Some(note_id), Some(row_id)) => write!(formatter, "id: {note_id}, row: {row_id}"),
            (Some(note_id), None) => write!(formatter, "id: {note_id}"),
            (None, Some(row_id)) => write!(formatter, "row: {row_id}"),
            (None, None) => formatter.write_str("identity: unknown"),
        }
    }
}

#[derive(Debug, Error)]
pub enum NtError {
    #[error("unknown help topic `{0}`; run nt help")]
    UnknownHelpTopic(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to {operation} `{path}`: {source}")]
    PathIo {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("operation committed but success output failed: {0}")]
    CommittedButOutputFailed(#[source] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[source] rusqlite::Error),
    #[error("failed to open database `{path}`: {source}")]
    OpenDatabase {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("database is busy; retry")]
    DatabaseBusy,
    #[error("database could not enter WAL mode")]
    WalUnavailable,
    #[error("database is corrupt")]
    CorruptDatabase(#[source] rusqlite::Error),
    #[error("invalid database path")]
    InvalidDatabasePath,
    #[error("system clock is outside the supported timestamp range")]
    ClockOutOfRange,
    #[error("stored note is invalid ({context}, field: {field})")]
    InvalidStoredNote {
        context: StoredNoteContext,
        field: &'static str,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    #[error("home directory not found")]
    HomeNotFound,
    #[error("run nt init first")]
    MissingDatabase,
    #[error("database is not an nt database")]
    NotNtDatabase,
    #[error("unsupported nt schema version {0}; migrate or initialize a compatible database")]
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
    #[error("failed to launch editor: {0}")]
    EditorLaunch(#[source] std::io::Error),
    #[error("editor exited unsuccessfully with status {0}")]
    EditorExit(std::process::ExitStatus),
    #[error("note changed while editing: {0}")]
    ConcurrentEdit(String),
    #[error("note revision conflict: {id} (expected {expected}, found {actual}); retry")]
    RevisionConflict {
        id: String,
        expected: u64,
        actual: u64,
    },
}

impl NtError {
    pub(crate) fn invalid_stored(context: StoredNoteContext, field: &'static str) -> Self {
        Self::InvalidStoredNote {
            context,
            field,
            source: None,
        }
    }

    pub(crate) fn invalid_stored_with_source(
        context: StoredNoteContext,
        field: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::InvalidStoredNote {
            context,
            field,
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn path_io(
        operation: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::PathIo {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn open_database(path: &Path, error: rusqlite::Error) -> Self {
        match Self::from(error) {
            Self::Database(source) => Self::OpenDatabase {
                path: path.to_path_buf(),
                source,
            },
            classified => classified,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::UnknownHelpTopic(_)
            | Self::InvalidNoteId(_)
            | Self::InvalidValue { .. }
            | Self::DuplicateNoteId(_)
            | Self::EmptyBody
            | Self::InvalidTitle
            | Self::SelfLink
            | Self::InvalidBodyVersion(_)
            | Self::ConflictingBodyInput
            | Self::EditorNotSet
            | Self::InvalidEditor => 2,
            Self::MissingDatabase | Self::NoteNotFound(_) => 3,
            Self::DatabaseBusy | Self::ConcurrentEdit(_) | Self::RevisionConflict { .. } => 4,
            Self::Io(_)
            | Self::PathIo { .. }
            | Self::CommittedButOutputFailed(_)
            | Self::Json(_)
            | Self::Database(_)
            | Self::OpenDatabase { .. }
            | Self::WalUnavailable
            | Self::CorruptDatabase(_)
            | Self::InvalidDatabasePath
            | Self::ClockOutOfRange
            | Self::InvalidStoredNote { .. }
            | Self::HomeNotFound
            | Self::NotNtDatabase
            | Self::UnsupportedSchema(_)
            | Self::EditorLaunch(_)
            | Self::EditorExit(_) => 1,
        }
    }
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
    use std::io;
    use std::path::Path;

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

    #[test]
    fn errors_have_stable_process_categories() {
        assert_eq!(NtError::InvalidTitle.exit_code(), 2);
        assert_eq!(NtError::DuplicateNoteId("id".to_string()).exit_code(), 2);
        assert_eq!(NtError::MissingDatabase.exit_code(), 3);
        assert_eq!(NtError::DatabaseBusy.exit_code(), 4);
        assert_eq!(
            NtError::Io(std::io::Error::other("unexpected")).exit_code(),
            1
        );
    }

    #[test]
    fn path_errors_preserve_context_sources_and_sqlite_categories() {
        let path = Path::new("/notes/nt.sqlite3");
        let error = NtError::path_io("inspect database path", path, io::Error::other("denied"));
        assert!(matches!(&error, NtError::PathIo { path: stored, .. } if stored == path));
        assert_eq!(
            error.to_string(),
            "failed to inspect database path `/notes/nt.sqlite3`: denied"
        );
        assert!(error.source().is_some());
        assert_eq!(error.exit_code(), 1);

        let error = NtError::open_database(path, sqlite_failure(rusqlite::ffi::SQLITE_IOERR));
        assert!(matches!(
            &error,
            NtError::OpenDatabase { path: stored, .. } if stored == path
        ));
        assert!(error.to_string().contains("/notes/nt.sqlite3"));
        assert!(error.source().is_some());

        assert!(matches!(
            NtError::open_database(path, sqlite_failure(rusqlite::ffi::SQLITE_BUSY)),
            NtError::DatabaseBusy
        ));
        assert!(matches!(
            NtError::open_database(path, sqlite_failure(rusqlite::ffi::SQLITE_CORRUPT)),
            NtError::CorruptDatabase(_)
        ));
    }

    fn sqlite_failure(code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
    }
}
