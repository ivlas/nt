use std::fmt;
use std::str::FromStr;

use uuid::{Uuid, Version};

use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoteId(Uuid);

impl NoteId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(value).map_err(|_| NtError::InvalidNoteId(value.to_string()))?;
        if uuid.get_version() != Some(Version::SortRand) || uuid.to_string() != value {
            return Err(NtError::InvalidNoteId(value.to_string()));
        }
        Ok(Self(uuid))
    }
}

#[cfg(test)]
mod tests {
    use super::NoteId;

    #[test]
    fn generates_canonical_uuid_v7_ids() {
        let id = NoteId::generate();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text.as_bytes()[14], b'7');
        assert_eq!(text.parse::<NoteId>().unwrap(), id);
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn rejects_noncanonical_and_non_v7_ids() {
        assert!("NT20260528T143012".parse::<NoteId>().is_err());
        assert!(
            "550e8400-e29b-41d4-a716-446655440000"
                .parse::<NoteId>()
                .is_err()
        );
        assert!(
            "018FBE0A-6C00-7000-8000-000000000001"
                .parse::<NoteId>()
                .is_err()
        );
    }
}
