use uuid::{Uuid, Version};

use crate::error::{NtError, Result};

pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn validate_id(id: &str) -> Result<()> {
    let uuid = Uuid::parse_str(id).map_err(|_| NtError::InvalidNoteId(id.to_string()))?;
    if uuid.get_version() != Some(Version::SortRand) || uuid.to_string() != id {
        return Err(NtError::InvalidNoteId(id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{new_id, validate_id};

    #[test]
    fn generates_canonical_uuid_v7_ids() {
        let id = new_id();
        validate_id(&id).unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(id.as_bytes()[14], b'7');
    }

    #[test]
    fn rejects_legacy_and_non_v7_ids() {
        assert!(validate_id("NT20260528T143012").is_err());
        assert!(validate_id("550e8400-e29b-41d4-a716-446655440000").is_err());
    }
}
