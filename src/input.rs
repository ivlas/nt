use std::io::{self, IsTerminal, Read, Write};
use std::process::Command;

use crate::error::{NtError, Result};
use crate::fs::nt_home;

pub fn read_body(arguments: &[String], seed: Option<&str>) -> Result<String> {
    let stdin_is_terminal = io::stdin().is_terminal();
    if !arguments.is_empty() {
        if !stdin_is_terminal {
            return Err(NtError::ConflictingBodyInput);
        }
        return Ok(arguments.join(" "));
    }

    if !stdin_is_terminal {
        let mut body = String::new();
        io::stdin().read_to_string(&mut body)?;
        if body.is_empty() {
            return Err(NtError::EmptyBody);
        }
        return Ok(body);
    }

    read_editor(seed)
}

fn read_editor(seed: Option<&str>) -> Result<String> {
    let editor = std::env::var("EDITOR").map_err(|_| NtError::EditorNotSet)?;
    if editor.trim().is_empty() {
        return Err(NtError::EditorNotSet);
    }

    let directory = nt_home()?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    if let Some(seed) = seed {
        file.write_all(seed.as_bytes())?;
        file.flush()?;
    }
    let path = file.path().to_string_lossy();
    let command = format!("{editor} \"{}\"", path.replace('"', "\\\""));
    let status = Command::new("sh").arg("-c").arg(command).status()?;
    if !status.success() {
        return Err(NtError::EditorFailed);
    }

    let body = std::fs::read_to_string(file.path())?;
    if body.is_empty() {
        return Err(NtError::EmptyBody);
    }
    Ok(body)
}
