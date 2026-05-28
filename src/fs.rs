use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

pub fn home_dir() -> Result<PathBuf> {
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }

    if let Some(home) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(home));
    }

    Err(NtError::HomeNotFound)
}

pub fn nt_home() -> Result<PathBuf> {
    Ok(home_dir()?.join(".nt"))
}

pub fn index_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("index.json"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("config.json"))
}

pub fn skills_dir() -> Result<PathBuf> {
    Ok(nt_home()?.join("skills"))
}

pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    Ok(env::current_dir()?.join(path))
}

pub fn relative_to_cwd(path: &Path) -> PathBuf {
    let Ok(cwd) = env::current_dir() else {
        return path.to_path_buf();
    };

    path.strip_prefix(cwd).unwrap_or(path).to_path_buf()
}

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

fn write_and_rename(tmp_path: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    {
        let mut file = File::create(tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    fs::rename(tmp_path, path)?;
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

    use super::atomic_write;

    #[test]
    fn atomic_write_replaces_file_contents() {
        let dir = std::env::temp_dir().join(format!("nt-test-{}", std::process::id()));
        let path = dir.join("note.md");

        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");

        let _ = fs::remove_dir_all(dir);
    }
}
