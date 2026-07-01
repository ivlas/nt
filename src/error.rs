use thiserror::Error;

#[derive(Debug, Error)]
pub enum NtError {
    #[error("{0}")]
    Message(String),
    #[error(
        "{operation} failed and rollback failed; manual repair needed: original error: {original}; rollback error: {rollback}"
    )]
    RollbackFailed {
        operation: &'static str,
        original: Box<NtError>,
        rollback: Box<NtError>,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
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
    #[error("note must start with a non-empty `# Title` heading")]
    InvalidTitle,
    #[error("editor failed: {0}")]
    EditorFailed(String),
}

impl NtError {
    pub fn rollback_failed(operation: &'static str, original: NtError, rollback: NtError) -> Self {
        Self::RollbackFailed {
            operation,
            original: Box::new(original),
            rollback: Box::new(rollback),
        }
    }
}

pub type Result<T> = std::result::Result<T, NtError>;

#[cfg(test)]
mod tests {
    use super::NtError;

    #[test]
    fn rollback_failed_message_includes_both_errors() {
        let err = NtError::rollback_failed(
            "saving index",
            NtError::Message("index write failed".to_string()),
            NtError::Message("note cleanup failed".to_string()),
        );

        let message = err.to_string();
        assert!(message.contains("saving index failed and rollback failed"));
        assert!(message.contains("original error: index write failed"));
        assert!(message.contains("rollback error: note cleanup failed"));
    }
}
