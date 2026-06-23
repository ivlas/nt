use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command as ProcessCommand;

use crate::error::{NtError, Result};

use super::paths::{index_lock_path, nt_home};

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
