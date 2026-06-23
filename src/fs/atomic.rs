use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let tmp_path = parent.join(tmp_name(path));
    let write_result = write_and_rename(&tmp_path, path, bytes);

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }

    write_result
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

fn write_and_rename(tmp_path: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    {
        let mut file = File::create(tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    fs::rename(tmp_path, path)?;
    sync_parent_dir(path)?;
    Ok(())
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

fn tmp_name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nt.tmp");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    format!(".{file_name}.{nanos}.tmp")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{atomic_write, create_new_file};

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
}
