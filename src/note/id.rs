use std::fmt;
use std::str::FromStr;

use uuid::{Uuid, Version};

use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoteId(String);

impl NoteId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NoteId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NoteId {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(value).map_err(|_| NtError::InvalidNoteId(value.to_string()))?;
        if uuid.get_version() != Some(Version::SortRand) || uuid.to_string() != value {
            return Err(NtError::InvalidNoteId(value.to_string()));
        }
        Ok(Self(value.to_string()))
    }
}

pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn validate_id(id: &str) -> Result<()> {
    id.parse::<NoteId>().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{NoteId, new_id, validate_id};

    #[test]
    fn generates_canonical_uuid_v7_ids() {
        let id = NoteId::generate();
        validate_id(id.as_str()).unwrap();
        assert_eq!(id.as_str().len(), 36);
        assert_eq!(id.as_str().as_bytes()[14], b'7');
        validate_id(&new_id()).unwrap();
    }

    #[test]
    fn rejects_legacy_and_non_v7_ids() {
        assert!(validate_id("NT20260528T143012").is_err());
        assert!(validate_id("550e8400-e29b-41d4-a716-446655440000").is_err());
    }
}
