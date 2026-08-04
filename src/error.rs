use thiserror::Error;

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataErrorKind {
    UnknownExpression,
    UnknownField,
    TodoOnly,
    EmptyValue,
    MultipleValues,
    DuplicateField,
    RequiresValue,
    RequiresSignedValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionErrorKind {
    MissingQualifier,
    InvalidVault,
    InvalidName,
}

#[derive(Debug, Error)]
pub enum NtError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(rusqlite::Error),
    #[error("database is busy; retry the command")]
    DatabaseBusy,
    #[error("invalid metadata")]
    InvalidMetadata {
        command: &'static str,
        field: Option<String>,
        value: Option<String>,
        kind: MetadataErrorKind,
    },
    #[error("invalid collection")]
    InvalidCollection {
        value: String,
        component: Option<String>,
        kind: CollectionErrorKind,
    },
    #[error("home directory not found")]
    HomeNotFound,
    #[error("run `nt init <vault>` first")]
    MissingVault,
    #[error("database at {} is not initialized by nt; refusing to overwrite or modify it", .0.display())]
    UninitializedDatabase(PathBuf),
    #[error(
        "database at {} is not a valid nt database; refusing to overwrite or modify it: {source}",
        path.display()
    )]
    InvalidDatabase {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("note not found: {0}")]
    NoteNotFound(String),
    #[error("invalid note id: {0}")]
    InvalidNoteId(String),
    #[error("empty note")]
    EmptyNote,
    #[error("note must start with a non-empty `# Title` heading")]
    InvalidTitle,
    #[error("editor failed: {0}")]
    EditorFailed(String),
    #[error("note changed during edit")]
    ConcurrentEdit { note_id: String },
    #[error("export failed")]
    ExportFailure {
        path: PathBuf,
        note_id: Option<String>,
        #[source]
        source: Box<NtError>,
    },
}

impl From<rusqlite::Error> for NtError {
    fn from(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(sqlite_error, _) = &error
            && matches!(
                sqlite_error.code,
                rusqlite::ffi::ErrorCode::DatabaseBusy | rusqlite::ffi::ErrorCode::DatabaseLocked
            )
        {
            Self::DatabaseBusy
        } else {
            Self::Database(error)
        }
    }
}

pub type Result<T> = std::result::Result<T, NtError>;
