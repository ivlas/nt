use std::fs::{self, File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

const TMP_CREATE_ATTEMPTS: usize = 16;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    with_temp_retry(path, bytes, tmp_nonce(), write_and_rename)
}

#[cfg(test)]
fn atomic_write_with_nonce(path: &Path, bytes: &[u8], nonce: u128) -> Result<()> {
    with_temp_retry(path, bytes, nonce, write_and_rename)
}

pub fn create_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    with_temp_retry(path, bytes, tmp_nonce(), write_and_rename_new)
}

#[cfg(test)]
fn create_new_file_with_nonce(path: &Path, bytes: &[u8], nonce: u128) -> Result<()> {
    with_temp_retry(path, bytes, nonce, write_and_rename_new)
}

fn with_temp_retry(
    path: &Path,
    bytes: &[u8],
    nonce: u128,
    rename_fn: fn(&Path, &Path, &[u8]) -> std::result::Result<(), TempWriteError>,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    for attempt in 0..TMP_CREATE_ATTEMPTS {
        let tmp_path = parent.join(tmp_name(path, nonce, attempt));
        match rename_fn(&tmp_path, path, bytes) {
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

fn write_and_rename(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), TempWriteError> {
    write_and_rename_with_sync(tmp_path, path, bytes, sync_parent_dir)
}

fn write_and_rename_with_sync(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
    sync: fn(&Path) -> std::io::Result<()>,
) -> std::result::Result<(), TempWriteError> {
    write_temp_file(tmp_path, bytes)?;
    fs::rename(tmp_path, path).map_err(NtError::from)?;
    sync(path).map_err(|source| NtError::write_committed_but_not_durable(path, source))?;
    Ok(())
}

fn write_and_rename_new(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), TempWriteError> {
    write_and_rename_new_with_sync(tmp_path, path, bytes, sync_parent_dir)
}

fn write_and_rename_new_with_sync(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
    sync: fn(&Path) -> std::io::Result<()>,
) -> std::result::Result<(), TempWriteError> {
    write_temp_file(tmp_path, bytes)?;

    if path.try_exists().map_err(NtError::from)? {
        let _ = fs::remove_file(tmp_path);
        return Err(TempWriteError::Other(
            Error::new(
                ErrorKind::AlreadyExists,
                format!("file exists: {}", path.display()),
            )
            .into(),
        ));
    }

    fs::rename(tmp_path, path).map_err(NtError::from)?;
    sync(path).map_err(|source| NtError::write_committed_but_not_durable(path, source))?;
    Ok(())
}

fn write_temp_file(tmp_path: &Path, bytes: &[u8]) -> std::result::Result<(), TempWriteError> {
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
fn sync_parent_dir(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "path has no parent"))?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
fn atomic_write_with_sync_failure(path: &Path, bytes: &[u8]) -> Result<()> {
    with_temp_retry(path, bytes, tmp_nonce(), write_and_rename_with_sync_failure)
}

#[cfg(test)]
fn write_and_rename_with_sync_failure(
    tmp_path: &Path,
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), TempWriteError> {
    write_and_rename_with_sync(tmp_path, path, bytes, |_| Err(Error::other("sync failed")))
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

    use super::{
        atomic_write, atomic_write_with_nonce, atomic_write_with_sync_failure, create_new_file,
        create_new_file_with_nonce, tmp_name,
    };

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
    fn create_new_file_retries_temp_collision_without_truncating_existing_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "nt-test-create-new-temp-collision-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        let tmp_path = dir.join(tmp_name(&path, 0, 0));

        fs::write(&tmp_path, "keep").unwrap();
        create_new_file_with_nonce(&path, b"new", 0).unwrap();

        assert_eq!(fs::read_to_string(&tmp_path).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");

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

    #[test]
    fn reports_uncertain_durability_after_rename() {
        let dir = std::env::temp_dir().join(format!(
            "nt-test-atomic-sync-failure-{}",
            std::process::id()
        ));
        let path = dir.join("note.md");

        let err = atomic_write_with_sync_failure(&path, b"committed").unwrap_err();

        assert!(err.is_write_committed_but_not_durable());
        assert_eq!(fs::read_to_string(&path).unwrap(), "committed");

        let _ = fs::remove_dir_all(dir);
    }
}
