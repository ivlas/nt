use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command as ProcessCommand;
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

fn index_lock_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("index.lock"))
}

pub struct IndexMutationLock {
    path: PathBuf,
}

impl IndexMutationLock {
    pub fn acquire() -> Result<Self> {
        let dir = nt_home()?;
        fs::create_dir_all(&dir)?;
        let path = index_lock_path()?;
        match create_index_lock(&path) {
            Ok(()) => Ok(Self { path }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if lock_holder_is_dead(&path)? {
                    fs::remove_file(&path)?;
                    create_index_lock(&path)
                        .map(|()| Self { path: path.clone() })
                        .map_err(|err| {
                            if err.kind() == std::io::ErrorKind::AlreadyExists {
                                lock_error(&path)
                            } else {
                                err.into()
                            }
                        })
                } else {
                    Err(lock_error(&path))
                }
            }
            Err(err) => Err(err.into()),
        }
    }
}

impl Drop for IndexMutationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_index_lock(path: &Path) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(file, "{}", std::process::id())?;
    file.sync_all()
}

fn lock_holder_is_dead(path: &Path) -> Result<bool> {
    let text = fs::read_to_string(path)?;
    let Some(pid) = text
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok())
    else {
        return Ok(false);
    };

    Ok(!pid_is_running(pid))
}

#[cfg(unix)]
fn pid_is_running(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(unix))]
fn pid_is_running(_pid: u32) -> bool {
    true
}

fn lock_error(path: &Path) -> NtError {
    NtError::Message(format!(
        "index is locked: {}; remove it if no nt mutation is running",
        path.display()
    ))
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
