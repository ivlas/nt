use std::env;
use std::path::{Path, PathBuf};

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

pub(super) fn index_lock_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("index.lock"))
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
