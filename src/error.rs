use thiserror::Error;

#[derive(Debug, Error)]
pub enum NtError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("toml encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
    #[error("home directory not found")]
    HomeNotFound,
    #[error("run `nt init <notes-dir>` first")]
    MissingVault,
    #[error("note not found: {0}")]
    NoteNotFound(String),
    #[error("invalid note id: {0}")]
    InvalidNoteId(String),
    #[error("empty note")]
    EmptyNote,
    #[error("editor failed: {0}")]
    EditorFailed(String),
}

pub type Result<T> = std::result::Result<T, NtError>;
