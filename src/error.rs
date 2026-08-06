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
    Database(rusqlite::Error),
    #[error("database is busy; retry")]
    DatabaseBusy,
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
    #[error("EDITOR is not set")]
    EditorNotSet,
    #[error("editor exited unsuccessfully")]
    EditorFailed,
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
