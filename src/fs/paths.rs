use std::env;
use std::path::PathBuf;

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

pub fn database_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("nt.sqlite3"))
}
