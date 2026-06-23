use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::{NtError, Result};
use crate::index::Index;

use super::date::timestamp_now;

pub fn iso_from_id(id: &str) -> Result<String> {
    validate_id(id)?;
    Ok(format!(
        "{}-{}-{}T{}:{}:{}Z",
        &id[2..6],
        &id[6..8],
        &id[8..10],
        &id[11..13],
        &id[13..15],
        &id[15..17]
    ))
}

pub fn generate_unique_id(notes_dir: &Path, index: &Index) -> Result<super::date::Timestamp> {
    for _ in 0..5 {
        let timestamp = timestamp_now();
        if !notes_dir.join(format!("{}.md", timestamp.id)).exists()
            && !index.notes.contains_key(&timestamp.id)
        {
            return Ok(timestamp);
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(NtError::Message(
        "could not allocate a unique note id".to_string(),
    ))
}

pub fn note_path(notes_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_id(id)?;
    Ok(notes_dir.join(format!("{id}.md")))
}

pub fn validate_id(id: &str) -> Result<()> {
    if id.len() != 17 || !id.starts_with("NT") {
        return Err(NtError::InvalidNoteId(id.to_string()));
    }

    let bytes = id.as_bytes();
    if bytes[10] != b'T' {
        return Err(NtError::InvalidNoteId(id.to_string()));
    }

    if id[2..10].chars().all(|char| char.is_ascii_digit())
        && id[11..17].chars().all(|char| char.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(NtError::InvalidNoteId(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_unique_id, validate_id};
    use crate::note::date::timestamp_now;

    #[test]
    fn validates_note_id_shape() {
        validate_id("NT20260528T143012").unwrap();
        assert!(validate_id("20260528T143012").is_err());
        assert!(validate_id("NT20260528-143012").is_err());
        assert!(validate_id("nt20260528T143012").is_err());
        assert!(validate_id("NT20260528T14301").is_err());
        assert!(validate_id("NT20260528T1430123").is_err());
        assert!(validate_id("NT2026AB28T143012").is_err());
        assert!(validate_id("NT20260528T14AB12").is_err());
        assert!(validate_id("NT20260528 143012").is_err());
    }

    #[test]
    fn generate_unique_id_retries_past_file_collision() {
        use crate::index::Index;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("nt-test-id-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let now = timestamp_now();
        fs::write(dir.join(format!("{}.md", now.id)), "# Collision\n").unwrap();

        let generated = generate_unique_id(&dir, &Index::default()).unwrap();
        assert_ne!(generated.id, now.id, "should retry past file collision");
        assert!(validate_id(&generated.id).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_unique_id_retries_past_index_collision() {
        use crate::index::{Index, NoteMeta};
        use std::fs;
        use std::path::PathBuf;

        let dir =
            std::env::temp_dir().join(format!("nt-test-index-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let now = timestamp_now();
        let mut index = Index::default();
        index.notes.insert(
            now.id.clone(),
            NoteMeta::new_note(
                now.id.clone(),
                PathBuf::from(format!("notes/{}.md", now.id)),
                now.iso.clone(),
                now.iso.clone(),
                "Collision".to_string(),
            ),
        );

        let generated = generate_unique_id(&dir, &index).unwrap();
        assert_ne!(generated.id, now.id, "should retry past index collision");
        assert!(validate_id(&generated.id).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }
}
