use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

const TMP_CREATE_ATTEMPTS: usize = 16;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with_nonce(path, bytes, tmp_nonce())
}

fn atomic_write_with_nonce(path: &Path, bytes: &[u8], nonce: u128) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    for attempt in 0..TMP_CREATE_ATTEMPTS {
        let tmp_path = parent.join(tmp_name(path, nonce, attempt));
        match write_and_rename(&tmp_path, path, bytes) {
            Ok(()) => return Ok(()),
            Err(TempWriteError::TempExists) => continue,
            Err(TempWriteError::Other(err)) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(err);
            }
        }
    }

    Err(NtError::Message(format!(
        "could not create temporary file for {} after {TMP_CREATE_ATTEMPTS} attempts",
        path.display()
    )))
}

pub fn create_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_parent_dir(path)?;
    Ok(())
}

fn write_and_rename(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), TempWriteError> {
    {
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(tmp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                return Err(TempWriteError::TempExists);
            }
            Err(err) => return Err(TempWriteError::Other(err.into())),
        };
        file.write_all(bytes).map_err(NtError::from)?;
        file.sync_all().map_err(NtError::from)?;
    }

    fs::rename(tmp_path, path).map_err(NtError::from)?;
    sync_parent_dir(path)?;
    Ok(())
}

#[derive(Debug)]
enum TempWriteError {
    TempExists,
    Other(NtError),
}

impl From<NtError> for TempWriteError {
    fn from(err: NtError) -> Self {
        Self::Other(err)
    }
}

#[cfg(unix)]
fn sync_parent_dir(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

fn tmp_name(path: &Path, nonce: u128, attempt: usize) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nt.tmp");

    format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        nonce,
        attempt
    )
}

fn tmp_nonce() -> u128 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    nanos
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{atomic_write, atomic_write_with_nonce, create_new_file, tmp_name};

    #[test]
    fn atomic_write_replaces_file_contents() {
        let dir = std::env::temp_dir().join(format!("nt-test-{}", std::process::id()));
        let path = dir.join("note.md");

        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn create_new_file_rejects_existing_paths() {
        let dir = std::env::temp_dir().join(format!("nt-test-create-new-{}", std::process::id()));
        let path = dir.join("note.md");

        create_new_file(&path, b"first").unwrap();
        assert!(create_new_file(&path, b"second").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn atomic_write_retries_temp_collision_without_truncating_existing_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "nt-test-atomic-temp-collision-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        let tmp_path = dir.join(tmp_name(&path, 0, 0));

        fs::write(&tmp_path, "keep").unwrap();
        atomic_write_with_nonce(&path, b"replace", 0).unwrap();

        assert_eq!(fs::read_to_string(&tmp_path).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&path).unwrap(), "replace");

        let _ = fs::remove_dir_all(dir);
    }
}
